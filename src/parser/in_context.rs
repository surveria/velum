use crate::error::Result;

use super::Parser;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum InOperatorContext {
    Allowed,
    Disallowed,
}

impl InOperatorContext {
    const fn from_allowed(allowed: bool) -> Self {
        if allowed {
            Self::Allowed
        } else {
            Self::Disallowed
        }
    }
}

impl Parser {
    pub(super) fn with_in_operator_allowed<T>(
        &mut self,
        allowed: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.in_operator_context;
        self.in_operator_context = InOperatorContext::from_allowed(allowed);
        let result = parse(self);
        self.in_operator_context = previous;
        result
    }

    pub(super) const fn in_operator_is_allowed(&self) -> bool {
        matches!(self.in_operator_context, InOperatorContext::Allowed)
    }
}
