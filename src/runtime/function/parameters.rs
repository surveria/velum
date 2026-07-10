use std::rc::Rc;

use crate::{
    binding_metadata::{BindingLayout, BindingOperand, ScopeId},
    bytecode::{BytecodeBlock, BytecodeFunction, BytecodeFunctionParam},
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::{BindingCell, BindingScope, BindingSlot},
        binding::static_bindings::{CompiledBindingFrame, StaticBindingCacheHandle},
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
    pub(super) fn function_param_atoms(
        &mut self,
        params: &[BytecodeFunctionParam],
    ) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            atoms.push(self.intern_static_name_atom(param.binding().name())?);
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

    pub(super) fn function_scope(
        &mut self,
        params: &[AtomId],
        frames: &[Option<CompiledBindingFrame>],
        args: &[Value],
        has_parameter_defaults: bool,
    ) -> Result<BindingScope> {
        if params.len() != frames.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        if !has_parameter_defaults
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
            let cell = if has_parameter_defaults {
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

    pub(super) fn eval_function_body(
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
                let default_layout = static_binding_layout.clone();
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
                        if let Some(completion) = context.apply_function_param_defaults(
                            parameters.binding_ids,
                            parameters.atoms,
                            bytecode.param_defaults(),
                            parameters.args,
                            Some(&default_layout),
                        )? {
                            return Ok(completion);
                        }
                        context
                            .hoist_bytecode_declarations(bytecode.hoist_plan())
                            .and_then(|()| {
                                context
                                    .eval_function_body_after_setup(parameters.function, bytecode)
                            })
                    },
                )
            }
            (Some(static_name_atom_cache), None, _) => {
                self.with_static_name_atom_cache(static_name_atom_cache, |context| {
                    if let Some(completion) = context.apply_function_param_defaults(
                        parameters.binding_ids,
                        parameters.atoms,
                        bytecode.param_defaults(),
                        parameters.args,
                        None,
                    )? {
                        return Ok(completion);
                    }
                    context
                        .hoist_bytecode_declarations(bytecode.hoist_plan())
                        .and_then(|()| {
                            context.eval_function_body_after_setup(parameters.function, bytecode)
                        })
                })
            }
            (None, _, _) | (Some(_), Some(_), None) => {
                if let Some(completion) = self.apply_function_param_defaults(
                    parameters.binding_ids,
                    parameters.atoms,
                    bytecode.param_defaults(),
                    parameters.args,
                    None,
                )? {
                    return Ok(completion);
                }
                self.hoist_bytecode_declarations(bytecode.hoist_plan())
                    .and_then(|()| {
                        self.eval_function_body_after_setup(parameters.function, bytecode)
                    })
            }
        }
    }

    fn eval_function_body_after_setup(
        &mut self,
        function: FunctionId,
        bytecode: &BytecodeFunction,
    ) -> Result<Completion> {
        if let Some(completion) = self.eval_bytecode_function_fast_path(bytecode)? {
            return Ok(completion);
        }
        self.eval_bytecode_function_body(function, bytecode.body())
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

    fn apply_function_param_defaults(
        &mut self,
        binding_ids: &[StaticBindingId],
        atoms: &[AtomId],
        defaults: &[Option<BytecodeBlock>],
        args: &[Value],
        layout: Option<&BindingLayout>,
    ) -> Result<Option<Completion>> {
        if binding_ids.len() != atoms.len() || binding_ids.len() != defaults.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        if !defaults.iter().any(Option::is_some) {
            return Ok(None);
        }
        for (index, ((binding, atom), default)) in binding_ids
            .iter()
            .copied()
            .zip(atoms.iter().copied())
            .zip(defaults.iter())
            .enumerate()
        {
            let cell = self.function_param_cell(binding, atom, layout)?;
            let argument = args.get(index).cloned().unwrap_or(Value::Undefined);
            let value = if default.is_some() && matches!(argument, Value::Undefined) {
                let Some(default) = default else {
                    return Err(Error::runtime("function parameter default disappeared"));
                };
                match self.eval_bytecode_block(default)? {
                    Completion::Normal(value) => value,
                    completion => return Ok(Some(completion)),
                }
            } else {
                self.runtime_value(argument)?
            };
            cell.initialize(value)?;
        }
        Ok(None)
    }

    fn function_param_cell(
        &self,
        binding: StaticBindingId,
        atom: AtomId,
        layout: Option<&BindingLayout>,
    ) -> Result<BindingCell> {
        if !self.has_visible_local_scope() {
            return Err(Error::runtime("function parameter scope is not active"));
        }
        let Some(scope) = self.locals.last() else {
            return Err(Error::runtime("function parameter scope is not active"));
        };
        if let Some(frame) = function_param_frame(binding, layout)? {
            return scope
                .cell_for_slot(atom, frame.slot())
                .ok_or_else(|| Error::runtime("function parameter binding is not defined"));
        }
        scope
            .get(atom)
            .ok_or_else(|| Error::runtime("function parameter binding is not defined"))
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
        args.push(packed);
        Ok(args)
    }
}

pub(super) fn function_param_binding_ids(
    params: &[BytecodeFunctionParam],
) -> Rc<[StaticBindingId]> {
    params
        .iter()
        .map(|param| param.binding().id())
        .collect::<Vec<_>>()
        .into()
}

pub(super) fn function_param_frames(
    params: &[BytecodeFunctionParam],
    layout: Option<&BindingLayout>,
) -> Result<Rc<[Option<CompiledBindingFrame>]>> {
    params
        .iter()
        .map(|param| function_param_frame(param.binding().id(), layout))
        .collect::<Result<Vec<_>>>()
        .map(Rc::from)
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
    has_parameter_defaults: bool,
) -> Result<Option<std::rc::Rc<super::FunctionScopeTemplate>>> {
    if has_parameter_defaults || params.len() != frames.len() || !params_have_unique_atoms(params) {
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
