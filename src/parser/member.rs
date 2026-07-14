use crate::{
    ast::{Expr, Expression},
    error::{Error, Result},
    lexer::TokenKind,
    syntax::StaticName,
    value::Value,
};

use super::{Parser, property_name::keyword_property_name};

impl Parser {
    pub(super) fn is_optional_chain(expr: &Expression) -> bool {
        match expr.kind() {
            Expr::OptionalChain(_)
            | Expr::OptionalMember { .. }
            | Expr::OptionalComputedMember { .. }
            | Expr::OptionalPrivateMember { .. }
            | Expr::OptionalCall { .. } => true,
            Expr::Member { object, .. }
            | Expr::ComputedMember { object, .. }
            | Expr::PrivateMember { object, .. } => Self::is_optional_chain(object),
            Expr::Call { callee, .. } => Self::is_optional_chain(callee),
            _ => false,
        }
    }

    /// Parses one `.name` or `.#name` member suffix after its consumed dot.
    pub(super) fn member_dot_suffix(&mut self, expr: Expression) -> Result<Expression> {
        let start = expr.span();
        if let Some(name) = self.match_private_name()? {
            return Ok(self.expression_node(
                start,
                Expr::PrivateMember {
                    object: Box::new(expr),
                    name,
                },
            ));
        }
        let property = self.consume_property_name("expected property name after '.'")?;
        let access = self.static_property_access()?;
        Ok(self.expression_node(
            start,
            Expr::Member {
                object: Box::new(expr),
                property,
                access,
            },
        ))
    }

    /// Parses a static optional member suffix after its consumed `?.` token.
    pub(super) fn optional_member_dot_suffix(&mut self, expr: Expression) -> Result<Expression> {
        let start = expr.span();
        let property = self.consume_property_name("expected property name after '?.'")?;
        let access = self.static_property_access()?;
        Ok(self.expression_node(
            start,
            Expr::OptionalMember {
                object: Box::new(expr),
                property,
                access,
            },
        ))
    }

    /// Parses the suffix introduced by a consumed `?.` token.
    pub(super) fn optional_chain_suffix(&mut self, expr: Expression) -> Result<Expression> {
        let start = expr.span();
        if self.match_kind(&TokenKind::LParen) {
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.arguments()?
            };
            self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
            let site = self.static_call_site()?;
            return Ok(self.expression_node(
                start,
                Expr::OptionalCall {
                    callee: Box::new(expr),
                    site,
                    strict: self.is_strict_mode(),
                    args,
                },
            ));
        }
        if self.match_kind(&TokenKind::LBracket) {
            let property = self.expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after property expression",
            )?;
            let access = self.static_property_access()?;
            return Ok(self.expression_node(
                start,
                Expr::OptionalComputedMember {
                    object: Box::new(expr),
                    property: Box::new(property),
                    access,
                },
            ));
        }
        if let Some(name) = self.match_private_name()? {
            return Ok(self.expression_node(
                start,
                Expr::OptionalPrivateMember {
                    object: Box::new(expr),
                    name,
                },
            ));
        }
        self.optional_member_dot_suffix(expr)
    }

    /// Parses one `[expression]` member suffix after its consumed bracket,
    /// folding literal keys into static member accesses.
    pub(super) fn member_bracket_suffix(&mut self, expr: Expression) -> Result<Expression> {
        let property = self.expression()?;
        self.consume(
            &TokenKind::RBracket,
            "expected ']' after property expression",
        )?;
        let access = self.static_property_access()?;
        let start = expr.span();
        if let Some(property) = self.static_computed_property_key(&property)? {
            return Ok(self.expression_node(
                start,
                Expr::Member {
                    object: Box::new(expr),
                    property,
                    access,
                },
            ));
        }
        Ok(self.expression_node(
            start,
            Expr::ComputedMember {
                object: Box::new(expr),
                property: Box::new(property),
                access,
            },
        ))
    }

    /// Consumes the next token when it is a `#name` private identifier,
    /// interning the name and recording its use for end-of-class validation.
    pub(super) fn match_private_name(&mut self) -> Result<Option<StaticName>> {
        if !matches!(self.peek_kind(0), Some(TokenKind::PrivateName(_))) {
            return Ok(None);
        }
        let token = self.advance_token("expected private name")?;
        let token_span = token.span;
        let TokenKind::PrivateName(text) = token.kind else {
            return Err(Error::parse_at("expected private name", token_span));
        };
        let name = self.static_name_shared(text)?;
        self.record_private_name_use(&name, token_span)?;
        Ok(Some(name))
    }

    /// Rejects `delete obj.#name`, including parenthesized forms, per the
    /// dedicated early error for private member deletion.
    pub(super) fn reject_private_delete_target(expr: &Expression) -> Result<()> {
        let mut current = expr;
        loop {
            match current.kind() {
                Expr::Parenthesized(inner) => current = inner,
                Expr::PrivateMember { .. } => {
                    return Err(Error::parse_at(
                        "private members cannot be deleted",
                        current.span(),
                    ));
                }
                _ => return Ok(()),
            }
        }
    }

    pub(super) fn is_identifier_reference(expr: &Expression) -> bool {
        let mut current = expr;
        loop {
            match current.kind() {
                Expr::Parenthesized(inner) => current = inner,
                Expr::Identifier(_) => return true,
                _ => return false,
            }
        }
    }

    pub(super) fn consume_property_name(&mut self, message: &str) -> Result<StaticName> {
        let token = self.advance_token(message)?;
        let token_span = token.span;
        match token.kind {
            TokenKind::Identifier(name) => self.static_name_shared(name),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse_at(message, token_span));
                };
                self.borrowed_static_name(name)
            }
        }
    }

    /// Folds literal computed keys such as `obj["text"]` into static member
    /// property names at parse time.
    fn static_computed_property_key(
        &mut self,
        property: &Expression,
    ) -> Result<Option<StaticName>> {
        match property.kind() {
            Expr::StringLiteral { value, .. } => {
                self.borrowed_static_name(value.as_str()).map(Some)
            }
            Expr::Literal(
                value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
            ) => self.static_name(value.to_string()).map(Some),
            _ => Ok(None),
        }
    }
}
