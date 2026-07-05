use crate::{
    ast::{Expr, Stmt},
    error::Result,
    runtime::Context,
    runtime_completion::Completion,
    value::Value,
};

impl Context {
    pub(crate) fn eval_while(&mut self, condition: &Expr, body: &Stmt) -> Result<Completion> {
        let mut last = Value::Undefined;
        while self.eval_expr(condition)?.is_truthy() {
            self.step()?;
            match self.eval_statement(body)? {
                Completion::Normal(value) => last = value,
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(completion);
                }
                Completion::Break => return Ok(Completion::Normal(last)),
                Completion::Continue => {}
            }
        }
        Ok(Completion::Normal(last))
    }

    pub(crate) fn eval_for(
        &mut self,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<Completion> {
        if let Some(init) = init {
            self.evaluate_for_init(init)?;
        }

        let mut last = Value::Undefined;
        loop {
            if !self.for_condition_is_truthy(condition)? {
                break;
            }
            self.step()?;
            match self.eval_statement(body)? {
                Completion::Normal(value) => last = value,
                Completion::Continue => {}
                Completion::Break => return Ok(Completion::Normal(last)),
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(completion);
                }
            }
            self.eval_for_update(update)?;
        }
        Ok(Completion::Normal(last))
    }

    fn evaluate_for_init(&mut self, init: &Stmt) -> Result<()> {
        match self.eval_statement(init)? {
            Completion::Normal(_) => Ok(()),
            completion => completion.into_result().map(|_| ()),
        }
    }

    fn for_condition_is_truthy(&mut self, condition: Option<&Expr>) -> Result<bool> {
        let Some(condition) = condition else {
            return Ok(true);
        };
        Ok(self.eval_expr(condition)?.is_truthy())
    }

    fn eval_for_update(&mut self, update: Option<&Expr>) -> Result<()> {
        if let Some(update) = update {
            self.eval_expr(update)?;
        }
        Ok(())
    }
}
