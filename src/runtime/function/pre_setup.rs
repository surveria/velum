use std::rc::Rc;

use crate::{
    binding_metadata::BindingLayout,
    error::Result,
    runtime::{CompiledBindingFrame, Context, control::Completion},
    value::{FunctionId, Value},
};

use super::{BytecodeFunctionInit, FunctionFastPath};

impl Context {
    pub(super) fn capture_function_environment(
        &self,
        init: &BytecodeFunctionInit<'_>,
        param_frames: &[Option<CompiledBindingFrame>],
        layout: Option<&BindingLayout>,
    ) -> Result<(
        crate::runtime::CapturedFunctionUpvalues,
        Rc<[Value]>,
        Option<FunctionFastPath>,
    )> {
        let fast_path = if self.current_with_environments().is_empty() {
            self.compile_optional_function_fast_path(init, param_frames)?
        } else {
            None
        };
        let upvalues = self.capture_function_upvalues(
            init.static_function_id,
            init.bytecode.capture_bindings(),
            layout,
        )?;
        let with_environments = Rc::from(self.current_with_environments());
        Ok((upvalues, with_environments, fast_path))
    }

    pub(super) fn try_eval_pre_setup_function_fast_path(
        &mut self,
        id: FunctionId,
        raw_args: &[crate::value::Value],
    ) -> Result<Option<Completion>> {
        let Some((fast_path, upvalues, atom_cache, binding_cache, binding_layout)) = ({
            let function = self.function(id)?;
            if !function.with_environments.is_empty() {
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
                )
            })
        }) else {
            return Ok(None);
        };
        let upvalues = upvalues.as_deref().unwrap_or(&[]);
        let active_layout = self.current_static_binding_layout();
        if binding_layout == active_layout {
            return self.eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues);
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
