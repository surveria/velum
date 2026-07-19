#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    ast::{Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::{IMPORT_META_PROPERTY_NAME, Parser};

impl Parser {
    pub(super) fn new_expr(&mut self) -> Result<Expression> {
        let new_span = self.previous_span();
        if self.match_kind(&TokenKind::Dot) {
            let expr = self.new_target_expr(new_span)?;
            return self.call_suffix(expr);
        }
        let import_meta = self.peek_kind_is(1, &TokenKind::Dot)
            && self.peek_token(2).is_some_and(|token| {
                token.is_unescaped_identifier_named(IMPORT_META_PROPERTY_NAME)
            });
        if self.check(&TokenKind::Import) && !import_meta {
            return Err(Error::parse_at(
                "import call cannot be used as a constructor",
                new_span,
            ));
        }
        let constructor = if self.match_kind(&TokenKind::New) {
            self.new_expr()?
        } else {
            let constructor = self.primary()?;
            self.member_suffix(constructor)?
        };
        if self.check(&TokenKind::QuestionDot) {
            return Err(Error::parse_at(
                "optional chains cannot directly follow new expressions",
                new_span,
            ));
        }
        let args = if self.match_kind(&TokenKind::LParen) {
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.with_in_operator_allowed(true, Self::arguments)?
            };
            self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
            args
        } else {
            Vec::new()
        };
        let expr = self.expression_node(
            new_span,
            Expr::New {
                constructor: Box::new(constructor),
                args,
            },
        );
        self.call_suffix(expr)
    }
}
