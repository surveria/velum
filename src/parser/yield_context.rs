use crate::error::Result;

use super::Parser;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum YieldExpressionContext {
    Allowed,
    Forbidden,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum YieldIdentifierContext {
    Allowed,
    Reserved,
}

impl Parser {
    pub(super) fn with_yield_expression<T>(
        &mut self,
        allowed: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.yield_expression_context;
        self.yield_expression_context = if allowed {
            YieldExpressionContext::Allowed
        } else {
            YieldExpressionContext::Forbidden
        };
        let result = parse(self);
        self.yield_expression_context = previous;
        result
    }

    pub(super) const fn yield_expression_is_allowed(&self) -> bool {
        matches!(
            self.yield_expression_context,
            YieldExpressionContext::Allowed
        )
    }

    pub(super) fn with_yield_identifier_reserved<T>(
        &mut self,
        reserved: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.yield_identifier_context;
        self.yield_identifier_context = if reserved {
            YieldIdentifierContext::Reserved
        } else {
            YieldIdentifierContext::Allowed
        };
        let result = parse(self);
        self.yield_identifier_context = previous;
        result
    }

    pub(super) const fn yield_identifier_is_reserved(&self) -> bool {
        matches!(
            self.yield_identifier_context,
            YieldIdentifierContext::Reserved
        ) || self.yield_expression_is_allowed()
    }
}
