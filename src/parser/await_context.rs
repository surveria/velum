use crate::error::Result;

use super::Parser;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum AwaitExpressionContext {
    Allowed,
    Forbidden,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum AwaitIdentifierContext {
    Allowed,
    Reserved,
}

impl Parser {
    pub(super) fn forbid_top_level_await_expression(&mut self) {
        self.await_expression_context = AwaitExpressionContext::Forbidden;
    }

    pub(super) fn with_function_await_context<T>(
        &mut self,
        expression_allowed: bool,
        is_async: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let identifier_reserved = is_async || self.is_module_goal();
        self.with_await_context(expression_allowed, identifier_reserved, parse)
    }

    pub(super) fn with_class_field_await_context<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let identifier_reserved = self.is_module_goal();
        self.with_await_context(false, identifier_reserved, parse)
    }

    pub(super) fn with_await_context<T>(
        &mut self,
        expression_allowed: bool,
        identifier_reserved: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = (self.await_expression_context, self.await_identifier_context);
        self.await_expression_context = if expression_allowed {
            AwaitExpressionContext::Allowed
        } else {
            AwaitExpressionContext::Forbidden
        };
        self.await_identifier_context = if identifier_reserved {
            AwaitIdentifierContext::Reserved
        } else {
            AwaitIdentifierContext::Allowed
        };
        let result = parse(self);
        self.await_expression_context = previous.0;
        self.await_identifier_context = previous.1;
        result
    }

    pub(super) fn with_await_identifier_reserved<T>(
        &mut self,
        reserved: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.await_identifier_context;
        self.await_identifier_context = if reserved {
            AwaitIdentifierContext::Reserved
        } else {
            AwaitIdentifierContext::Allowed
        };
        let result = parse(self);
        self.await_identifier_context = previous;
        result
    }

    pub(super) const fn await_expression_is_allowed(&self) -> bool {
        matches!(
            self.await_expression_context,
            AwaitExpressionContext::Allowed
        )
    }

    pub(super) const fn await_identifier_is_reserved(&self) -> bool {
        matches!(
            self.await_identifier_context,
            AwaitIdentifierContext::Reserved
        )
    }
}
