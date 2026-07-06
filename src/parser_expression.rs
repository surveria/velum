use crate::{
    ast::{Expr, ObjectProperty, StaticBinding, StaticName, UnaryOp, UpdateOp},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::Parser;

const THIS_PROPERTY_NAME: &str = "this";

struct ObjectPropertyName {
    key: StaticName,
    shorthand_name: Option<StaticName>,
}

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
                let access = self.static_property_access()?;
                expr = Expr::Member {
                    object: Box::new(expr),
                    property,
                    access,
                };
                continue;
            }
            if self.match_kind(&TokenKind::LBracket) {
                let property = self.expression()?;
                self.consume(
                    &TokenKind::RBracket,
                    "expected ']' after property expression",
                )?;
                if let Some(property) = self.static_computed_property_key(&property)? {
                    let access = self.static_property_access()?;
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                        access,
                    };
                    continue;
                }
                let access = self.static_property_access()?;
                expr = Expr::ComputedMember {
                    object: Box::new(expr),
                    property: Box::new(property),
                    access,
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
        let constructor =
            self.consume_binding_identifier("expected constructor name after 'new'")?;
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
            TokenKind::Identifier(name) => Expr::Identifier(self.static_binding_name(name)?),
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
            properties.push(self.object_literal_property()?);
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

    fn object_literal_property(&mut self) -> Result<ObjectProperty> {
        let name = self.object_property_key()?;
        if self.match_kind(&TokenKind::Colon) {
            let value = self.expression()?;
            return Ok(ObjectProperty {
                key: name.key,
                value,
            });
        }
        if self.match_kind(&TokenKind::LParen) {
            let params = self.function_parameters()?.into();
            self.consume(&TokenKind::RParen, "expected ')' after method parameters")?;
            self.consume(&TokenKind::LBrace, "expected '{' before method body")?;
            let body = self.block_statements()?.into();
            let id = self.static_function()?;
            let value = Expr::MethodFunction {
                id,
                name: name.key.clone(),
                params,
                body,
            };
            return Ok(ObjectProperty {
                key: name.key,
                value,
            });
        }
        if let Some(binding) = name.shorthand_name {
            let binding = self.static_binding(binding)?;
            return Ok(ObjectProperty {
                key: name.key,
                value: Expr::Identifier(binding),
            });
        }
        Err(Error::parse(
            "expected ':' after object property name",
            self.offset(),
        ))
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

    fn object_property_key(&mut self) -> Result<ObjectPropertyName> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse("expected object property name", self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => {
                let name = self.static_name(name)?;
                Ok(ObjectPropertyName {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::String(name) => Ok(ObjectPropertyName {
                key: self.static_name(name)?,
                shorthand_name: None,
            }),
            TokenKind::Number(value) => Ok(ObjectPropertyName {
                key: self.static_name(Value::Number(value).to_string())?,
                shorthand_name: None,
            }),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse("expected object property name", token.offset));
                };
                self.keyword_property_name(name)
            }
        }
    }

    fn keyword_property_name(&mut self, name: &str) -> Result<ObjectPropertyName> {
        Ok(ObjectPropertyName {
            key: self.borrowed_static_name(name)?,
            shorthand_name: None,
        })
    }

    fn function_expression(&mut self) -> Result<Expr> {
        let name = if self.next_is_identifier() {
            Some(self.consume_identifier("expected function name")?)
        } else {
            None
        };
        self.consume(&TokenKind::LParen, "expected '(' after 'function'")?;
        let params = self.function_parameters()?.into();
        self.consume(&TokenKind::RParen, "expected ')' after function parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before function body")?;
        let body = self.block_statements()?.into();
        let id = self.static_function()?;
        Ok(Expr::Function {
            id,
            name,
            params,
            body,
        })
    }

    fn function_parameters(&mut self) -> Result<Vec<StaticBinding>> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            let name = self.consume_binding_identifier("expected function parameter name")?;
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

    fn consume_property_name(&mut self, message: &str) -> Result<StaticName> {
        let token = self
            .advance()
            .ok_or_else(|| Error::parse(message, self.offset()))?;
        match token.kind {
            TokenKind::Identifier(name) => self.static_name(name),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse(message, token.offset));
                };
                self.borrowed_static_name(name)
            }
        }
    }

    fn static_computed_property_key(&mut self, property: &Expr) -> Result<Option<StaticName>> {
        let name = match property {
            Expr::Literal(Value::String(value)) => value.clone(),
            Expr::Literal(
                value @ (Value::Undefined | Value::Null | Value::Bool(_) | Value::Number(_)),
            ) => value.to_string(),
            _ => return Ok(None),
        };
        self.static_name(name).map(Some)
    }
}

const fn keyword_property_name(kind: &TokenKind) -> Option<&'static str> {
    match kind {
        TokenKind::This => Some(THIS_PROPERTY_NAME),
        TokenKind::Let => Some("let"),
        TokenKind::Const => Some("const"),
        TokenKind::Var => Some("var"),
        TokenKind::If => Some("if"),
        TokenKind::Else => Some("else"),
        TokenKind::While => Some("while"),
        TokenKind::For => Some("for"),
        TokenKind::Switch => Some("switch"),
        TokenKind::Case => Some("case"),
        TokenKind::Default => Some("default"),
        TokenKind::Break => Some("break"),
        TokenKind::Continue => Some("continue"),
        TokenKind::Try => Some("try"),
        TokenKind::Catch => Some("catch"),
        TokenKind::Finally => Some("finally"),
        TokenKind::Throw => Some("throw"),
        TokenKind::Return => Some("return"),
        TokenKind::Function => Some("function"),
        TokenKind::New => Some("new"),
        TokenKind::In => Some("in"),
        TokenKind::Typeof => Some("typeof"),
        TokenKind::Void => Some("void"),
        TokenKind::Delete => Some("delete"),
        TokenKind::True => Some("true"),
        TokenKind::False => Some("false"),
        TokenKind::Null => Some("null"),
        TokenKind::Undefined => Some("undefined"),
        _ => None,
    }
}
