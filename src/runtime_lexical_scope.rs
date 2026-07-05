use crate::{
    ast::Stmt,
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    value::Value,
};

impl Context {
    pub(crate) fn eval_declaration_list(&mut self, declarations: &[Stmt]) -> Result<Completion> {
        let mut last = Value::Undefined;
        for declaration in declarations {
            match self.eval_statement(declaration)? {
                Completion::Normal(value) => last = value,
                completion => return Ok(completion),
            }
        }
        Ok(Completion::Normal(last))
    }

    pub(crate) fn eval_scoped_block(&mut self, statements: &[Stmt]) -> Result<Completion> {
        self.with_lexical_scope(|context| context.eval_block(statements))
    }

    pub(crate) fn with_lexical_scope<T>(
        &mut self,
        evaluate: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.push_lexical_scope();
        let result = evaluate(self);
        let removed = self.pop_lexical_scope();
        if removed.is_none() {
            return Err(Error::runtime("lexical scope disappeared"));
        }
        result
    }
}
