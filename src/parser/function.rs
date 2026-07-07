use crate::{ast::FunctionParam, error::Result, lexer::TokenKind};

use super::Parser;

impl Parser {
    pub(super) fn function_parameters(&mut self) -> Result<Vec<FunctionParam>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }
            let name = self.consume_binding_identifier("expected function parameter name")?;
            let default = if self.match_kind(&TokenKind::Equal) {
                Some(self.assignment()?)
            } else {
                None
            };
            params.push(FunctionParam::new(name, default));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        self.reject_duplicate_non_simple_parameters(&params)?;
        Ok(params)
    }
}
