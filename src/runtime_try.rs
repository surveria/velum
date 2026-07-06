use crate::{
    ast::{CatchClause, DeclKind, StaticBinding, Stmt},
    error::Result,
    runtime::{CompiledBindingFrame, Context},
    runtime_completion::Completion,
    runtime_scope::BindingCell,
    value::Value,
};

impl Context {
    pub(crate) fn eval_try(
        &mut self,
        body: &[Stmt],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Stmt]>,
    ) -> Result<Completion> {
        let completion = self.eval_try_body(body, catch)?;
        let Some(finally_body) = finally_body else {
            return Ok(completion);
        };

        let finally_completion = self.eval_scoped_block(finally_body)?;
        if matches!(finally_completion, Completion::Normal(_)) {
            Ok(completion)
        } else {
            Ok(finally_completion)
        }
    }

    fn eval_try_body(&mut self, body: &[Stmt], catch: Option<&CatchClause>) -> Result<Completion> {
        match self.eval_scoped_block(body)? {
            Completion::Throw(value) => {
                let Some(catch) = catch else {
                    return Ok(Completion::Throw(value));
                };
                self.eval_catch(catch, value)
            }
            completion => Ok(completion),
        }
    }

    fn eval_catch(&mut self, catch: &CatchClause, value: Value) -> Result<Completion> {
        let Some(param) = catch.param.as_ref() else {
            return self.eval_scoped_block(&catch.body);
        };
        self.with_lexical_scope(|context| context.eval_catch_scope(catch, param, value))
    }

    fn eval_catch_scope(
        &mut self,
        catch: &CatchClause,
        param: &StaticBinding,
        value: Value,
    ) -> Result<Completion> {
        let atom = self.ensure_binding_capacity_static(param)?;
        let frame = self.compiled_local_binding_frame(param)?;
        let value = self.runtime_value(value)?;
        let inserted = self
            .active_bindings_mut()
            .insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, true, DeclKind::Let),
                frame.map(CompiledBindingFrame::slot),
            )?;
        self.mark_active_binding_frame_slot(frame, inserted)?;
        self.remember_active_static_binding(param, atom)?;
        self.eval_scoped_block(&catch.body)
    }
}
