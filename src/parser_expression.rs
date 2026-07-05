use crate::{
    ast::{Expr, ObjectProperty, UnaryOp, UpdateOp},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::Parser;

const THIS_PROPERTY_NAME: &str = "this";

impl Parser {
    pub(super) fn expression(&mut self) -> Result<Expr> {
        self.with_expression_depth(Self::assignment)
    }

    pub(super) fn unary(&mut self) -> Result<Expr> {
        if self.match_kind(&TokenKind::New) {
            return self.new_expr();
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            let offset = self.previous_offset();
            let expr = self.unary()?;
            return Self::update_expr(UpdateOp::Increment, true, expr, offset);
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            let offset = self.previous_offset();
            let expr = self.unary()?;
            return Self::update_expr(UpdateOp::Decrement, true, expr, offset);
        }
        if self.match_kind(&TokenKind::Typeof) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Typeof,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Void) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Void,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Delete) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Delete,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Bang) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Minus) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                expr: Box::new(expr),
            });
        }
        if self.match_kind(&TokenKind::Plus) {
            let expr = self.unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
            });
        }
        self.call()
    }

    pub(super) fn call(&mut self) -> Result<Expr> {
        let mut expr = self.primary()?;
        loop {
            if self.match_kind(&TokenKind::Dot) {
                let property = self.consume_property_name("expected property name after '.'")?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                };
                continue;
            }
            if self.match_kind(&TokenKind::LBracket) {
                let property = self.expression()?;
                self.consume(
                    &TokenKind::RBracket,
                    "expected ']' after property expression",
                )?;
                expr = Expr::ComputedMember {
                    object: Box::new(expr),
                    property: Box::new(property),
                };
                continue;
            }
            if !self.match_kind(&TokenKind::LParen) {
                break;
            }
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.arguments()?
            };
            self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
            expr = Expr::Call {
                callee: Box::new(expr),
                args,
            };
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            return Self::update_expr(UpdateOp::Increment, false, expr, self.previous_offset());
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            return Self::update_expr(UpdateOp::Decrement, false, expr, self.previous_offset());
        }
        Ok(expr)
    }

    pub(super) fn assignment_target(expr: Expr) -> Option<Expr> {
        match expr {
            Expr::Identifier(_) | Expr::Member { .. } | Expr::ComputedMember { .. } => Some(expr),
            Expr::Parenthesized(expr) => Self::assignment_target(*expr),
            _ => None,
        }
    }

    fn new_expr(&mut self) -> Result<Expr> {
        let constructor = self.consume_identifier("expected constructor name after 'new'")?;
        self.consume(&TokenKind::LParen, "expected '(' after constructor name")?;
        let args = if self.check(&TokenKind::RParen) {
            Vec::new()
        } else {
            self.arguments()?
        };
        self.consume(&TokenKind::RParen, "expected ')' after arguments")?;
        Ok(Expr::New { constructor, args })
    }

    fn update_expr(op: UpdateOp, prefix: bool, expr: Expr, offset: usize) -> Result<Expr> {
        let expr = Self::assignment_target(expr)
            .ok_or_else(|| Error::parse("invalid update target", offset))?;
        Ok(Expr::Update {
            op,
            prefix,
            expr: Box::new(expr),
        })
    }

    fn arguments(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();
        loop {
            args.push(self.expression()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected expression", self.offset()))?;
        let expr = match token.kind {
            TokenKind::Number(value) => Expr::Literal(Value::Number(value)),
            TokenKind::String(value) => Expr::Literal(Value::String(value)),
            TokenKind::True => Expr::Literal(Value::Bool(true)),
            TokenKind::False => Expr::Literal(Value::Bool(false)),
            TokenKind::Null => Expr::Literal(Value::Null),
            TokenKind::Undefined => Expr::Literal(Value::Undefined),
            TokenKind::This => Expr::This,
            TokenKind::Identifier(name) => Expr::Identifier(name),
            TokenKind::Function => self.function_expression()?,
            TokenKind::LBrace => self.object_literal()?,
            TokenKind::LBracket => self.array_literal()?,
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')' after expression")?;
                Expr::Parenthesized(Box::new(expr))
            }
            _ => return Err(Error::parse("expected expression", token.offset)),
        };
        Ok(expr)
    }

    fn object_literal(&mut self) -> Result<Expr> {
        let mut properties = Vec::new();
        if self.match_kind(&TokenKind::RBrace) {
            return Ok(Expr::Object(properties));
        }

        loop {
            let key = self.object_property_key()?;
            self.consume(&TokenKind::Colon, "expected ':' after object property name")?;
            let value = self.expression()?;
            properties.push(ObjectProperty { key, value });
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBrace) {
                return Ok(Expr::Object(properties));
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}' after object literal")?;
        Ok(Expr::Object(properties))
    }

    fn array_literal(&mut self) -> Result<Expr> {
        let mut elements = Vec::new();
        if self.match_kind(&TokenKind::RBracket) {
            return Ok(Expr::Array(elements));
        }

        loop {
            elements.push(self.expression()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBracket) {
                return Ok(Expr::Array(elements));
            }
        }

        self.consume(&TokenKind::RBracket, "expected ']' after array literal")?;
        Ok(Expr::Array(elements))
    }

    fn object_property_key(&mut self) -> Result<String> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected object property name", self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) | TokenKind::String(name) => Ok(name),
            TokenKind::This => Ok(THIS_PROPERTY_NAME.to_owned()),
            _ => Err(Error::parse("expected object property name", token.offset)),
        }
    }

    fn function_expression(&mut self) -> Result<Expr> {
        let name = if self.next_is_identifier() {
            Some(self.consume_identifier("expected function name")?)
        } else {
            None
        };
        self.consume(&TokenKind::LParen, "expected '(' after 'function'")?;
        let params = self.function_parameters()?;
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.block_statements()?;
        Ok(Expr::Function { name, params, body })
    }

    fn function_parameters(&mut self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            let name = self.consume_identifier("expected function parameter name")?;
            params.push(name);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }

    fn with_expression_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<Expr>,
    ) -> Result<Expr> {
        self.expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("expression nesting overflowed"))?;
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

    fn consume_property_name(&mut self, message: &str) -> Result<String> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse(message, self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            TokenKind::This => Ok(THIS_PROPERTY_NAME.to_owned()),
            _ => Err(Error::parse(message, token.offset)),
        }
    }
}
