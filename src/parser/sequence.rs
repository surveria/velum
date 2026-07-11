use crate::{
    ast::{Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::Parser;

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expression> {
        self.with_expression_depth(Self::sequence_expression)
    }

    pub(super) fn assignment_expression(&mut self) -> Result<Expression> {
        self.with_expression_depth(Self::assignment)
    }

    fn sequence_expression(&mut self) -> Result<Expression> {
        let first = self.assignment()?;
        if !self.match_kind(&TokenKind::Comma) {
            return Ok(first);
        }
        let start = first.span();
        let mut expressions = vec![first];
        loop {
            expressions.push(self.assignment()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(self.expression_node(start, Expr::Sequence(expressions)))
    }

    fn with_expression_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Expression>,
    ) -> Result<Expression> {
        self.expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("expression nesting overflowed"))?;
        self.max_expression_depth = self.max_expression_depth.max(self.expression_depth);
        if self.expression_depth > self.limits.max_expression_depth {
            self.expression_depth = self.expression_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "expression nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.expression_depth = self.expression_depth.saturating_sub(1);
        result
    }
}
