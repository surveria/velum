#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    binding_metadata::{BindingLayout, BindingOperand},
    bytecode::BytecodeBinding,
    error::Result,
    runtime::activation::DynamicEnvironment,
    runtime::{CompiledBindingFrame, Context, control::Completion},
    syntax::StaticBinding,
    value::{FunctionId, Value},
};

use super::{
    BytecodeFunctionInit, FunctionFastPath,
    fast_path::{FastStoreTarget, FastValueSource, FunctionFastPathKind},
};

impl Context {
    pub(super) fn fast_pre_setup_load_global_binding(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Value> {
        if let Some(atom) = self.lookup_static_name_atom(binding.name().name())?
            && !self.realm.object_global_names.contains(&atom)
            && let Some(cell) = self
                .realm
                .globals
                .get(atom)
                .or_else(|| self.realm.builtin_globals.get(atom))
        {
            return self.runtime_value(cell.value(binding.name())?);
        }
        self.unresolved_global_property_value(binding.name().name())?
            .ok_or_else(|| crate::runtime::control::reference_error_undefined(binding.name()))
    }

    pub(super) fn capture_function_environment(
        &mut self,
        init: &BytecodeFunctionInit<'_>,
        param_frames: &[Option<CompiledBindingFrame>],
        layout: Option<&BindingLayout>,
    ) -> Result<(
        crate::runtime::CapturedFunctionUpvalues,
        Rc<[DynamicEnvironment]>,
        Option<FunctionFastPath>,
    )> {
        let fast_path = if self.current_dynamic_environments().is_empty() {
            self.compile_optional_function_fast_path(init, param_frames)?
        } else {
            None
        };
        let upvalues = self.capture_function_upvalues(
            init.static_function_id,
            init.bytecode.capture_bindings(),
            layout,
        )?;
        let mut dynamic_environments = self.current_dynamic_environments().to_vec();
        let requires_unbounded_capture = init.bytecode.requires_dynamic_lexical_capture();
        let created_during_direct_eval = self.direct_eval_binding_layout_is_active();
        if (requires_unbounded_capture || created_during_direct_eval)
            && let Some(environment) = self.capture_direct_eval_lexical_environment(
                (!requires_unbounded_capture).then(|| init.bytecode.capture_bindings()),
            )?
        {
            dynamic_environments.insert(0, DynamicEnvironment::CapturedLexical(environment));
        }
        Ok((upvalues, dynamic_environments.into(), fast_path))
    }

    fn capture_direct_eval_lexical_environment(
        &mut self,
        referenced_bindings: Option<&[StaticBinding]>,
    ) -> Result<Option<crate::runtime::activation::EvalBindingEnvironment>> {
        let referenced_atoms = referenced_bindings
            .map(|bindings| {
                bindings
                    .iter()
                    .map(|binding| self.intern_static_name_atom(binding.name()))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let environment = self.create_eval_binding_environment()?;
        for scope in self
            .locals
            .iter()
            .skip(self.current_local_frame_start())
            .rev()
        {
            scope.for_each_active_binding(|atom, cell| {
                if referenced_atoms
                    .as_ref()
                    .is_some_and(|atoms| !atoms.contains(&atom))
                {
                    return Ok(());
                }
                if !self.dynamic_eval_environment_contains(atom)? && !environment.contains(atom)? {
                    environment.insert(atom, cell.clone(), false)?;
                }
                Ok(())
            })?;
        }
        if environment.len()? == 0 {
            return Ok(None);
        }
        Ok(Some(environment))
    }

    fn dynamic_eval_environment_contains(
        &self,
        atom: crate::storage::atom::AtomId,
    ) -> Result<bool> {
        self.current_dynamic_environments()
            .iter()
            .try_fold(false, |found, environment| {
                if found {
                    return Ok(true);
                }
                let DynamicEnvironment::EvalBindings(environment) = environment else {
                    return Ok(false);
                };
                environment.contains(atom)
            })
    }

    pub(super) fn try_eval_pre_setup_function_fast_path(
        &mut self,
        id: FunctionId,
        raw_args: &[crate::value::Value],
    ) -> Result<Option<Completion>> {
        let Some((fast_path, upvalues, atom_cache, binding_cache, binding_layout, dynamic_source)) =
            ({
                let function = self.function(id)?;
                if !function.dynamic_environments.is_empty() {
                    return Ok(None);
                }
                function.fast_path.as_ref().map(|fast_path| {
                    let upvalues = fast_path
                        .needs_upvalues()
                        .then(|| Rc::clone(&function.upvalues));
                    (
                        Rc::clone(fast_path),
                        upvalues,
                        function.static_name_atom_cache.clone(),
                        function.static_binding_cache.clone(),
                        function.static_binding_layout.clone(),
                        function.source.is_some(),
                    )
                })
            })
        else {
            return Ok(None);
        };
        let upvalues = upvalues.as_deref().unwrap_or(&[]);
        let active_layout = self.current_static_binding_layout();
        let binding_layout_is_active = match (&binding_layout, &active_layout) {
            (Some(binding_layout), Some(active_layout)) => {
                binding_layout.shares_operand_storage_with(active_layout)
                    || binding_layout == active_layout
            }
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        };
        if binding_layout_is_active {
            return self.eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues);
        }
        if !self.active_function_has_arguments_binding() {
            if fast_path_static_caches_are_compatible(&fast_path.kind, self)? {
                return self
                    .eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues);
            }
            if !dynamic_source {
                return Ok(None);
            }
        }
        match (atom_cache, binding_cache, binding_layout) {
            (Some(atom_cache), Some(binding_cache), Some(binding_layout)) => self
                .with_static_name_caches(atom_cache, binding_cache, binding_layout, |context| {
                    context
                        .eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues)
                }),
            (Some(atom_cache), _, _) => self.with_static_name_atom_cache(atom_cache, |context| {
                context.eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues)
            }),
            (None, _, _) => {
                self.eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues)
            }
        }
    }
}

fn fast_path_static_caches_are_compatible(
    kind: &FunctionFastPathKind,
    context: &Context,
) -> Result<bool> {
    match kind {
        FunctionFastPathKind::ReturnLiteral(_)
        | FunctionFastPathKind::ReturnString(_)
        | FunctionFastPathKind::ReturnUndefined => Ok(true),
        FunctionFastPathKind::ReturnSource(source) => {
            fast_source_static_caches_are_compatible(source, context)
        }
        FunctionFastPathKind::ReturnNumberBinary { left, right, .. }
        | FunctionFastPathKind::ReturnNumberCompare { left, right, .. }
        | FunctionFastPathKind::ReturnNumberEquality { left, right, .. } => {
            Ok(fast_source_static_caches_are_compatible(left, context)?
                && fast_source_static_caches_are_compatible(right, context)?)
        }
        FunctionFastPathKind::StoreNumberBinaryReturn {
            target,
            left,
            right,
            ..
        } => Ok(fast_target_static_caches_are_compatible(target, context)?
            && fast_source_static_caches_are_compatible(left, context)?
            && fast_source_static_caches_are_compatible(right, context)?),
    }
}

fn fast_source_static_caches_are_compatible(
    source: &FastValueSource,
    context: &Context,
) -> Result<bool> {
    match source {
        FastValueSource::Param(_) | FastValueSource::Literal(_) => Ok(true),
        FastValueSource::Binding(binding) => {
            if matches!(binding.operand(), BindingOperand::Upvalue { .. }) {
                return Ok(true);
            }
            context.active_static_caches_are_compatible(binding)
        }
        FastValueSource::NumberBinary { left, right, .. } => {
            Ok(fast_source_static_caches_are_compatible(left, context)?
                && fast_source_static_caches_are_compatible(right, context)?)
        }
    }
}

fn fast_target_static_caches_are_compatible(
    target: &FastStoreTarget,
    context: &Context,
) -> Result<bool> {
    match target {
        FastStoreTarget::Param => Ok(true),
        FastStoreTarget::Binding(binding) => {
            if matches!(binding.operand(), BindingOperand::Upvalue { .. }) {
                return Ok(true);
            }
            context.active_static_caches_are_compatible(binding)
        }
    }
}
