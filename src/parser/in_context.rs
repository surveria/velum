use crate::error::Result;

use super::Parser;

impl Parser {
    pub(super) fn with_in_operator_allowed<T>(
        &mut self,
        allowed: bool,
        parse: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.in_operator_allowed;
        self.in_operator_allowed = allowed;
        let result = parse(self);
        self.in_operator_allowed = previous;
        result
    }

    pub(super) const fn in_operator_is_allowed(&self) -> bool {
        self.in_operator_allowed
    }
}
