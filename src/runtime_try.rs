use crate::{
    ast::{CatchClause, DeclKind, Stmt},
    error::{Error, Result},
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

        let finally_completion = self.eval_block(finally_body)?;
        if matches!(finally_completion, Completion::Normal(_)) {
            Ok(completion)
        } else {
            Ok(finally_completion)
        }
    }

    fn eval_try_body(&mut self, body: &[Stmt], catch: Option<&CatchClause>) -> Result<Completion> {
        match self.eval_block(body)? {
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
        let previous = self.active_bindings_mut().remove(&catch.param);
        if previous.is_none() {
            self.ensure_binding_capacity(&catch.param)?;
        }
        self.checked_value(value.clone())?;
        self.active_bindings_mut().insert(
            catch.param.clone(),
            BindingCell::new(value, true, DeclKind::Let),
        );
        let result = self.eval_block(&catch.body);
        let removed = self.active_bindings_mut().remove(&catch.param);
        if removed.is_none() {
            return Err(Error::runtime("catch binding disappeared"));
        }
        if let Some(previous) = previous {
            self.active_bindings_mut()
                .insert(catch.param.clone(), previous);
        }
        result
    }
}
