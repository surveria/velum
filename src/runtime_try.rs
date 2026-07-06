use crate::{
    ast::{CatchClause, DeclKind, StaticBinding, Stmt},
    error::Result,
    runtime::Context,
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
        self.checked_value(value.clone())?;
        self.active_bindings_mut()
            .insert(atom, BindingCell::new(value, true, DeclKind::Let));
        self.remember_active_static_binding(param, atom)?;
        self.eval_scoped_block(&catch.body)
    }
}
