use std::rc::Rc;

use crate::{
    binding_metadata::{BindingLayout, BindingOperand, ScopeId},
    bytecode::{
        BytecodeDestructureMode, BytecodeFunction, BytecodeFunctionParam,
        BytecodeFunctionParamTarget,
    },
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::{BindingCell, BindingScope, BindingSlot},
        binding::static_bindings::{CompiledBindingFrame, StaticBindingCacheHandle},
        bytecode::{DestructureOutcome, state::BytecodeState},
        control::Completion,
        property::static_names::StaticNameAtomCacheHandle,
    },
    storage::atom::AtomId,
    syntax::{DeclKind, StaticBindingId},
    value::{FunctionId, Value},
};

#[derive(Clone, Copy)]
pub(super) struct FunctionParameterState<'a> {
    function: FunctionId,
    binding_ids: &'a [StaticBindingId],
    atoms: &'a [AtomId],
    args: &'a [Value],
}

impl<'a> FunctionParameterState<'a> {
    pub(super) const fn new(
        function: FunctionId,
        binding_ids: &'a [StaticBindingId],
        atoms: &'a [AtomId],
        args: &'a [Value],
    ) -> Self {
        Self {
            function,
            binding_ids,
            atoms,
            args,
        }
    }
}

impl Context {
    pub(super) fn compile_function_self_binding(
        &mut self,
        bytecode: &BytecodeFunction,
        layout: Option<&BindingLayout>,
    ) -> Result<Option<super::FunctionSelfBinding>> {
        bytecode
            .self_binding()
            .map(|binding| {
                let atom = self.intern_static_name_atom(binding.name())?;
                let frame = function_self_binding_frame(binding.id(), layout)?;
                Ok(super::FunctionSelfBinding::new(atom, frame))
            })
            .transpose()
    }

    pub(super) fn compile_function_arguments_binding(
        &mut self,
        bytecode: &BytecodeFunction,
        layout: Option<&BindingLayout>,
    ) -> Result<Option<super::FunctionArgumentsBinding>> {
        if !bytecode.uses_arguments() {
            return Ok(None);
        }
        bytecode
            .arguments_binding()
            .map(|binding| {
                let atom = self.intern_static_name_atom(binding.name())?;
                let frame = function_arguments_binding_frame(binding.id(), layout)?;
                Ok(super::FunctionArgumentsBinding::new(
                    atom,
                    frame,
                    bytecode.strict() || !bytecode.simple_parameters,
                ))
            })
            .transpose()
    }

    pub(super) fn function_param_atoms(
        &mut self,
        params: &[BytecodeFunctionParam],
    ) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            param.for_each_binding(&mut |binding| {
                atoms.push(self.intern_static_name_atom(binding.name().name())?);
                Ok(())
            })?;
        }
        Ok(atoms.into())
    }

    /// Builds the call scope from the shared per-function template: one
    /// exactly sized value-slot allocation, no per-call hashing or sorting.
    pub(super) fn function_scope_from_template(
        &mut self,
        template: &FunctionScopeTemplate,
        args: &[Value],
    ) -> Result<BindingScope> {
        self.ensure_extra_binding_capacity(template.param_count)?;
        let mut slots = Vec::with_capacity(template.param_count);
        for index in 0..template.param_count {
            let value = args.get(index).cloned().unwrap_or(Value::Undefined);
            slots.push(BindingCell::new(
                self.runtime_value(value)?,
                true,
                DeclKind::Var,
            ));
        }
        Ok(BindingScope::from_shared_template(
            template.scope,
            std::rc::Rc::clone(&template.index),
            slots,
        ))
    }

    pub(super) fn function_call_scope(
        &mut self,
        template: Option<&FunctionScopeTemplate>,
        params: &[AtomId],
        frames: &[Option<CompiledBindingFrame>],
        args: &[Value],
        requires_parameter_initialization: bool,
    ) -> Result<BindingScope> {
        match template {
            Some(template) => self.function_scope_from_template(template, args),
            None => self.function_scope(params, frames, args, requires_parameter_initialization),
        }
    }

    pub(super) fn function_scope(
        &mut self,
        params: &[AtomId],
        frames: &[Option<CompiledBindingFrame>],
        args: &[Value],
        requires_parameter_initialization: bool,
    ) -> Result<BindingScope> {
        if params.len() != frames.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        if !requires_parameter_initialization
            && params_have_unique_atoms(params)
            && let Some(scope) = contiguous_parameter_scope(frames)
        {
            self.ensure_extra_binding_capacity(params.len())?;
            let mut slots = Vec::with_capacity(params.len());
            for (index, atom) in params.iter().copied().enumerate() {
                let value = args.get(index).cloned().unwrap_or(Value::Undefined);
                slots.push((
                    atom,
                    BindingCell::new(self.runtime_value(value)?, true, DeclKind::Var),
                ));
            }
            return BindingScope::from_compiled_slots(scope, slots);
        }
        let mut scope = BindingScope::new();
        for (index, (atom, frame)) in params
            .iter()
            .copied()
            .zip(frames.iter().copied())
            .enumerate()
        {
            if !scope.contains(atom) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let cell = if requires_parameter_initialization {
                BindingCell::uninitialized(true, DeclKind::Var)
            } else {
                let value = args.get(index).cloned().unwrap_or(Value::Undefined);
                BindingCell::new(self.runtime_value(value)?, true, DeclKind::Var)
            };
            if let Some(frame) = frame {
                let inserted = scope.insert_or_replace_at_slot(atom, cell, frame.slot())?;
                Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
            } else {
                scope.insert(atom, cell)?;
            }
        }
        Ok(scope)
    }

    pub(super) fn arguments_binding_scope(
        &mut self,
        function: FunctionId,
        binding: FunctionArgumentsBinding,
        original_args: &[Value],
    ) -> Result<BindingScope> {
        let frame = binding.frame();
        let scope = frame
            .scope()
            .ok_or_else(|| Error::runtime("arguments binding scope is not local"))?;
        if frame.slot().index() != 0 {
            return Err(Error::runtime(
                "arguments binding is not the first arguments-scope slot",
            ));
        }
        self.ensure_extra_binding_capacity(1)?;
        let cell = BindingCell::new(
            self.create_arguments_object(function, binding.unmapped(), original_args)?,
            true,
            DeclKind::Var,
        );
        BindingScope::from_compiled_slots(scope, vec![(binding.atom(), cell)])
    }

    pub(super) fn eval_function_body<const CAN_SUSPEND: bool>(
        &mut self,
        static_name_atom_cache: Option<StaticNameAtomCacheHandle>,
        static_binding_cache: Option<StaticBindingCacheHandle>,
        static_binding_layout: Option<BindingLayout>,
        parameters: FunctionParameterState<'_>,
        bytecode: &BytecodeFunction,
        remember_params: bool,
    ) -> Result<Completion> {
        match (
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
        ) {
            (
                Some(static_name_atom_cache),
                Some(static_binding_cache),
                Some(static_binding_layout),
            ) => {
                self.with_static_name_caches(
                    static_name_atom_cache,
                    static_binding_cache,
                    static_binding_layout,
                    |context| {
                        // Parameter slots never move between calls; the
                        // per-function binding cache stays warm after the
                        // first call.
                        if remember_params {
                            context.remember_function_params(
                                parameters.binding_ids,
                                parameters.atoms,
                            )?;
                        }
                        if let Some(completion) =
                            context.apply_function_parameters(bytecode, parameters.args)?
                        {
                            return Ok(completion);
                        }
                        context
                            .hoist_bytecode_declarations(bytecode.hoist_plan())
                            .and_then(|()| {
                                context.eval_function_body_after_setup::<CAN_SUSPEND>(
                                    parameters.function,
                                    bytecode,
                                )
                            })
                    },
                )
            }
            (Some(static_name_atom_cache), None, _) => {
                self.with_static_name_atom_cache(static_name_atom_cache, |context| {
                    if let Some(completion) =
                        context.apply_function_parameters(bytecode, parameters.args)?
                    {
                        return Ok(completion);
                    }
                    context
                        .hoist_bytecode_declarations(bytecode.hoist_plan())
                        .and_then(|()| {
                            context.eval_function_body_after_setup::<CAN_SUSPEND>(
                                parameters.function,
                                bytecode,
                            )
                        })
                })
            }
            (None, _, _) | (Some(_), Some(_), None) => {
                if let Some(completion) =
                    self.apply_function_parameters(bytecode, parameters.args)?
                {
                    return Ok(completion);
                }
                self.hoist_bytecode_declarations(bytecode.hoist_plan())
                    .and_then(|()| {
                        self.eval_function_body_after_setup::<CAN_SUSPEND>(
                            parameters.function,
                            bytecode,
                        )
                    })
            }
        }
    }

    fn eval_function_body_after_setup<const CAN_SUSPEND: bool>(
        &mut self,
        function: FunctionId,
        bytecode: &BytecodeFunction,
    ) -> Result<Completion> {
        if self.optional_optimizations_enabled()
            && self.current_with_environments().is_empty()
            && let Some(completion) = self.eval_bytecode_function_fast_path(bytecode)?
        {
            self.charge_runtime_steps(bytecode.body().instructions().len())?;
            return Ok(completion);
        }
        self.eval_bytecode_function_body::<CAN_SUSPEND>(function, bytecode.body())
    }

    fn remember_function_params(
        &self,
        binding_ids: &[StaticBindingId],
        atoms: &[AtomId],
    ) -> Result<()> {
        if binding_ids.len() != atoms.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        for (binding, atom) in binding_ids.iter().copied().zip(atoms.iter().copied()) {
            self.remember_active_static_binding_id(binding, atom)?;
        }
        Ok(())
    }

    fn apply_function_parameters(
        &mut self,
        bytecode: &BytecodeFunction,
        args: &[Value],
    ) -> Result<Option<Completion>> {
        if !bytecode.requires_parameter_initialization() {
            return Ok(None);
        }
        for (index, parameter) in bytecode.params().iter().enumerate() {
            let argument = args.get(index).cloned().unwrap_or(Value::Undefined);
            let value = if matches!(argument, Value::Undefined)
                && let Some(default) = parameter.default()
            {
                match self.eval_bytecode_block(default)? {
                    Completion::Normal(value) => value,
                    completion => return Ok(Some(completion)),
                }
            } else {
                self.runtime_value(argument)?
            };
            match parameter.target() {
                BytecodeFunctionParamTarget::Binding(binding) => {
                    self.initialize_bytecode_parameter(binding, value)?;
                }
                BytecodeFunctionParamTarget::Pattern(pattern) => {
                    let mut state =
                        BytecodeState::with_private_environment(self.current_private_environment());
                    match self.eval_resumable_destructure(
                        &mut state,
                        pattern,
                        BytecodeDestructureMode::Parameter,
                        Some(value),
                    )? {
                        DestructureOutcome::Completed => {}
                        DestructureOutcome::Abrupt(completion)
                            if completion.suspends_execution() =>
                        {
                            return Err(Error::runtime(
                                "function parameter destructuring unexpectedly suspended",
                            ));
                        }
                        DestructureOutcome::Abrupt(completion) => return Ok(Some(completion)),
                    }
                }
            }
        }
        Ok(None)
    }
}

impl Context {
    /// Repacks call arguments for a trailing rest parameter: positional
    /// arguments stay in place and the remainder binds as one array value.
    pub(super) fn pack_rest_arguments(
        &mut self,
        params: &[BytecodeFunctionParam],
        mut args: Vec<Value>,
    ) -> Result<Vec<Value>> {
        let Some(last) = params.last() else {
            return Ok(args);
        };
        if !last.rest() {
            return Ok(args);
        }
        let rest_index = params.len().saturating_sub(1);
        let rest = if args.len() > rest_index {
            args.split_off(rest_index)
        } else {
            Vec::new()
        };
        let packed = self.create_array_from_elements(rest)?;
        while args.len() < rest_index {
            args.push(Value::Undefined);
        }
        args.push(packed);
        Ok(args)
    }
}

pub(super) fn function_param_binding_ids(
    params: &[BytecodeFunctionParam],
) -> Result<Rc<[StaticBindingId]>> {
    let mut bindings = Vec::with_capacity(params.len());
    for param in params {
        param.for_each_binding(&mut |binding| {
            bindings.push(binding.name().id());
            Ok(())
        })?;
    }
    Ok(bindings.into())
}

pub(super) fn function_param_frames(
    params: &[BytecodeFunctionParam],
    layout: Option<&BindingLayout>,
) -> Result<Rc<[Option<CompiledBindingFrame>]>> {
    let mut frames = Vec::with_capacity(params.len());
    for param in params {
        param.for_each_binding(&mut |binding| {
            frames.push(function_param_frame(binding.name().id(), layout)?);
            Ok(())
        })?;
    }
    Ok(frames.into())
}

pub(super) fn function_arity(params: &[BytecodeFunctionParam]) -> super::super::FunctionArity {
    let arity = params
        .iter()
        .take_while(|param| !param.has_default() && !param.rest())
        .count();
    super::super::FunctionArity::new(arity)
}

fn function_param_frame(
    binding: StaticBindingId,
    layout: Option<&BindingLayout>,
) -> Result<Option<CompiledBindingFrame>> {
    let Some(layout) = layout else {
        return Ok(None);
    };
    let Some(operand) = layout.operand_for_binding_id(binding)? else {
        return Ok(None);
    };
    match operand {
        BindingOperand::Local { scope, slot } => Ok(Some(CompiledBindingFrame::local(
            scope,
            BindingSlot::from_index(slot.index()?),
        ))),
        BindingOperand::Global { .. } | BindingOperand::Upvalue { .. } => Err(Error::runtime(
            "function parameter binding layout is not a local slot",
        )),
        BindingOperand::Unresolved => Ok(None),
    }
}

/// Precomputed per-function call-scope layout: the shared atom index plus
/// the compiled scope id, so each call allocates only the value slots.
#[derive(Debug)]
pub(in crate::runtime) struct FunctionScopeTemplate {
    pub(super) scope: ScopeId,
    pub(super) index: std::rc::Rc<crate::runtime::binding::scope::ScopeIndexData>,
    pub(super) param_count: usize,
}

/// Precomputed local slot for a function's implicit `arguments` binding.
#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) struct FunctionArgumentsBinding {
    atom: AtomId,
    frame: CompiledBindingFrame,
    unmapped: bool,
}

impl FunctionArgumentsBinding {
    pub(super) const fn new(atom: AtomId, frame: CompiledBindingFrame, unmapped: bool) -> Self {
        Self {
            atom,
            frame,
            unmapped,
        }
    }

    pub(super) const fn atom(self) -> AtomId {
        self.atom
    }

    pub(super) const fn frame(self) -> CompiledBindingFrame {
        self.frame
    }

    pub(super) const fn unmapped(self) -> bool {
        self.unmapped
    }
}

/// Precomputed local slot for a named function expression's private
/// immutable self binding.
#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) struct FunctionSelfBinding {
    atom: AtomId,
    frame: CompiledBindingFrame,
}

impl FunctionSelfBinding {
    pub(super) const fn new(atom: AtomId, frame: CompiledBindingFrame) -> Self {
        Self { atom, frame }
    }

    pub(super) const fn atom(self) -> AtomId {
        self.atom
    }

    pub(super) const fn frame(self) -> CompiledBindingFrame {
        self.frame
    }
}

pub(super) fn function_self_binding_frame(
    binding: StaticBindingId,
    layout: Option<&BindingLayout>,
) -> Result<CompiledBindingFrame> {
    let layout = layout.ok_or_else(|| {
        Error::runtime("named function binding requires a compiled binding layout")
    })?;
    let operand = layout
        .operand_for_binding_id(binding)?
        .ok_or_else(|| Error::runtime("named function binding layout is not defined"))?;
    match operand {
        BindingOperand::Local { scope, slot } => Ok(CompiledBindingFrame::local(
            scope,
            BindingSlot::from_index(slot.index()?),
        )),
        BindingOperand::Global { .. }
        | BindingOperand::Upvalue { .. }
        | BindingOperand::Unresolved => Err(Error::runtime(
            "named function binding layout is not a local slot",
        )),
    }
}

pub(super) fn function_arguments_binding_frame(
    binding: StaticBindingId,
    layout: Option<&BindingLayout>,
) -> Result<CompiledBindingFrame> {
    let layout = layout
        .ok_or_else(|| Error::runtime("arguments binding requires a compiled binding layout"))?;
    let operand = layout
        .operand_for_binding_id(binding)?
        .ok_or_else(|| Error::runtime("arguments binding layout is not defined"))?;
    match operand {
        BindingOperand::Local { scope, slot } => Ok(CompiledBindingFrame::local(
            scope,
            BindingSlot::from_index(slot.index()?),
        )),
        BindingOperand::Global { .. }
        | BindingOperand::Upvalue { .. }
        | BindingOperand::Unresolved => Err(Error::runtime(
            "arguments binding layout is not a local slot",
        )),
    }
}

impl FunctionScopeTemplate {
    pub(in crate::runtime) fn storage_entry_count(&self) -> Result<usize> {
        self.index.storage_entry_count()
    }
}

/// Builds the shared per-function scope template when the parameter layout
/// is contiguous and unique; general layouts fall back to per-call
/// construction.
pub(super) fn function_scope_template(
    params: &[AtomId],
    frames: &[Option<CompiledBindingFrame>],
    requires_parameter_initialization: bool,
) -> Result<Option<std::rc::Rc<super::FunctionScopeTemplate>>> {
    if requires_parameter_initialization
        || params.len() != frames.len()
        || !params_have_unique_atoms(params)
    {
        return Ok(None);
    }
    let Some(scope) = contiguous_parameter_scope(frames) else {
        return Ok(None);
    };
    let index = crate::runtime::binding::scope::ScopeIndexData::from_slot_atoms(params)?;
    Ok(Some(std::rc::Rc::new(FunctionScopeTemplate {
        scope,
        index: std::rc::Rc::new(index),
        param_count: params.len(),
    })))
}

fn contiguous_parameter_scope(frames: &[Option<CompiledBindingFrame>]) -> Option<ScopeId> {
    let mut expected_scope = None;
    for (index, frame) in frames.iter().copied().enumerate() {
        let frame = frame?;
        let scope = frame.scope()?;
        if frame.slot().index() != index {
            return None;
        }
        if let Some(expected) = expected_scope {
            if expected != scope {
                return None;
            }
        } else {
            expected_scope = Some(scope);
        }
    }
    expected_scope
}

fn params_have_unique_atoms(params: &[AtomId]) -> bool {
    for (index, atom) in params.iter().enumerate() {
        if params
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == atom)
        {
            return false;
        }
    }
    true
}

impl super::super::FunctionArity {
    pub(super) fn value(self) -> Result<Value> {
        let length = u32::try_from(self.as_usize())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(Value::Number(f64::from(length)))
    }
}
