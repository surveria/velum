use crate::{
    ast::{Expr, SwitchCase},
    error::Result,
    runtime::Context,
    runtime_completion::Completion,
    value::Value,
};

impl Context {
    pub(crate) fn eval_switch(
        &mut self,
        discriminant: &Expr,
        cases: &[SwitchCase],
    ) -> Result<Completion> {
        let discriminant = self.eval_expr(discriminant)?;
        let Some(start) = self.switch_start_index(&discriminant, cases)? else {
            return Ok(Completion::Normal(Value::Undefined));
        };

        self.with_lexical_scope(|context| context.eval_switch_cases(cases, start))
    }

    fn eval_switch_cases(&mut self, cases: &[SwitchCase], start: usize) -> Result<Completion> {
        let mut last = Value::Undefined;
        for case in cases.iter().skip(start) {
            match self.eval_block(&case.statements)? {
                Completion::Normal(value) => last = value,
                Completion::Break => return Ok(Completion::Normal(last)),
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Continue) => {
                    return Ok(completion);
                }
            }
        }
        Ok(Completion::Normal(last))
    }

    pub(crate) fn hoist_switch_vars(&mut self, cases: &[SwitchCase]) -> Result<()> {
        for case in cases {
            self.hoist_var_declarations(&case.statements)?;
        }
        Ok(())
    }

    fn switch_start_index(
        &mut self,
        discriminant: &Value,
        cases: &[SwitchCase],
    ) -> Result<Option<usize>> {
        let mut default_index = None;
        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_index = Some(index);
                continue;
            };
            if self.eval_expr(test)? == *discriminant {
                return Ok(Some(index));
            }
        }
        Ok(default_index)
    }
}
