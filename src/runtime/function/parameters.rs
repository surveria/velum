use std::rc::Rc;

use crate::{
    binding_metadata::{BindingLayout, BindingOperand},
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
    value::Value,
};

#[derive(Clone, Copy)]
pub(super) struct FunctionParameterState<'a> {
    binding_ids: &'a [StaticBindingId],
    atoms: &'a [AtomId],
    args: &'a [Value],
}

impl<'a> FunctionParameterState<'a> {
    pub(super) const fn new(
        binding_ids: &'a [StaticBindingId],
        atoms: &'a [AtomId],
        args: &'a [Value],
    ) -> Self {
        Self {
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

    pub(super) fn function_scope(
        &mut self,
        params: &[AtomId],
        binding_ids: &[StaticBindingId],
        layout: Option<&BindingLayout>,
        args: &[Value],
        has_parameter_defaults: bool,
    ) -> Result<BindingScope> {
        if params.len() != binding_ids.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        let mut scope = BindingScope::new();
        for (index, (atom, binding)) in params
            .iter()
            .copied()
            .zip(binding_ids.iter().copied())
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
            if let Some(frame) = function_param_frame(binding, layout)? {
                let inserted = scope.insert_or_replace_at_slot(atom, cell, frame.slot())?;
                Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
            } else {
                scope.insert(atom, cell);
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
                        context
                            .remember_function_params(parameters.binding_ids, parameters.atoms)?;
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
                            .and_then(|()| context.eval_bytecode_block(bytecode.body()))
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
                        .and_then(|()| context.eval_bytecode_block(bytecode.body()))
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
                    .and_then(|()| self.eval_bytecode_block(bytecode.body()))
            }
        }
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

impl super::super::FunctionArity {
    pub(super) fn value(self) -> Result<Value> {
        let length = u32::try_from(self.as_usize())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(Value::Number(f64::from(length)))
    }
}
