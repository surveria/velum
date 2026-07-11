use crate::error::Result;

use super::Parser;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum AwaitExpressionContext {
    Allowed,
    Forbidden,
}

impl Parser {
    pub(super) fn with_await_expression<T>(
        &mut self,
        allowed: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.await_expression_context;
        self.await_expression_context = if allowed {
            AwaitExpressionContext::Allowed
        } else {
            AwaitExpressionContext::Forbidden
        };
        let result = parse(self);
        self.await_expression_context = previous;
        result
    }

    pub(super) const fn await_expression_is_allowed(&self) -> bool {
        matches!(
            self.await_expression_context,
            AwaitExpressionContext::Allowed
        )
    }
}
