use crate::{
    ast::{Expr, Expression, FunctionKind, FunctionParam, Statement, Stmt, UnaryOp, UpdateOp},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::{Parser, SUPER_IDENTIFIER_NAME};

const NEW_TARGET_PROPERTY_NAME: &str = "target";
const IMPORT_BINDING_NAME: &str = "import";

#[derive(Debug, Clone, Copy)]
enum ArrowParameters {
    Single,
    Parenthesized,
}

#[derive(Debug, Clone, Copy)]
struct ArrowSignature {
    is_async: bool,
    parameters: ArrowParameters,
}

impl Parser {
    pub(super) fn unary(&mut self) -> Result<Expression> {
        let start = self.current_span();
        if self.await_expression_is_allowed() && self.match_kind(&TokenKind::Await) {
            let expr = self.unary()?;
            return Ok(self.expression_node(start, Expr::Await(Box::new(expr))));
        }
        if self.match_kind(&TokenKind::New) {
            return self.new_expr();
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            let operator = self.previous_span();
            let expr = self.unary()?;
            return self.update_expr(UpdateOp::Increment, true, expr, operator);
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            let operator = self.previous_span();
            let expr = self.unary()?;
            return self.update_expr(UpdateOp::Decrement, true, expr, operator);
        }
        if self.match_kind(&TokenKind::Typeof) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Typeof,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Void) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Void,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Delete) {
            let expr = self.unary()?;
            Self::reject_private_delete_target(&expr)?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Delete,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Bang) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Tilde) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::BitNot,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Minus) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Negate,
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Plus) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Plus,
                    expr: Box::new(expr),
                },
            ));
        }
        self.call()
    }

    pub(super) fn call(&mut self) -> Result<Expression> {
        let expr = self.primary()?;
        self.call_suffix(expr)
    }

    fn call_suffix(&mut self, mut expr: Expression) -> Result<Expression> {
        loop {
            if self.match_kind(&TokenKind::Dot) {
                expr = self.member_dot_suffix(expr)?;
                continue;
            }
            if self.match_kind(&TokenKind::LBracket) {
                expr = self.member_bracket_suffix(expr)?;
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
            let site = self.static_call_site()?;
            let start = expr.span();
            expr = self.expression_node(
                start,
                Expr::Call {
                    callee: Box::new(expr),
                    site,
                    strict: self.is_strict_mode(),
                    args,
                },
            );
        }
        if self.match_kind(&TokenKind::PlusPlus) {
            return self.update_expr(UpdateOp::Increment, false, expr, self.previous_span());
        }
        if self.match_kind(&TokenKind::MinusMinus) {
            return self.update_expr(UpdateOp::Decrement, false, expr, self.previous_span());
        }
        Ok(expr)
    }

    pub(super) fn assignment_target(expr: Expression) -> Option<Expression> {
        let span = expr.span();
        match expr.into_kind() {
            kind @ (Expr::Identifier(_)
            | Expr::Member { .. }
            | Expr::ComputedMember { .. }
            | Expr::PrivateMember { .. }) => Some(Expression::new(kind, span)),
            Expr::Parenthesized(inner) => Self::assignment_target(*inner),
            _ => None,
        }
    }

    fn new_expr(&mut self) -> Result<Expression> {
        let new_span = self.previous_span();
        if self.match_kind(&TokenKind::Dot) {
            let expr = self.new_target_expr(new_span)?;
            return self.call_suffix(expr);
        }
        let constructor = if self.match_kind(&TokenKind::Import) {
            self.import_constructor_seed()?
        } else {
            self.primary()?
        };
        let constructor = self.member_suffix(constructor)?;
        if Self::constructor_starts_with_import(&constructor) {
            return Err(Error::parse_at(
                "import call cannot be used as a constructor",
                new_span,
            ));
        }
        let args = if self.match_kind(&TokenKind::LParen) {
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.arguments()?
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

    fn import_constructor_seed(&mut self) -> Result<Expression> {
        let start = self.previous_span();
        let name = self.borrowed_static_name(IMPORT_BINDING_NAME)?;
        let binding = self.static_binding(name)?;
        Ok(self.expression_node(start, Expr::Identifier(binding)))
    }

    fn constructor_starts_with_import(expr: &Expression) -> bool {
        match expr.kind() {
            Expr::Identifier(name) => name.as_str() == IMPORT_BINDING_NAME,
            Expr::Member { object, .. } | Expr::ComputedMember { object, .. } => {
                Self::constructor_starts_with_import(object)
            }
            Expr::Parenthesized(expr) => Self::constructor_starts_with_import(expr),
            _ => false,
        }
    }

    fn member_suffix(&mut self, mut expr: Expression) -> Result<Expression> {
        loop {
            if self.match_kind(&TokenKind::Dot) {
                expr = self.member_dot_suffix(expr)?;
                continue;
            }
            if !self.match_kind(&TokenKind::LBracket) {
                break;
            }
            expr = self.member_bracket_suffix(expr)?;
        }
        Ok(expr)
    }

    fn new_target_expr(&mut self, new_span: crate::SourceSpan) -> Result<Expression> {
        let token = self
            .advance()
            .ok_or_else(|| self.parse_error("expected 'target' after 'new.'"))?;
        let token_span = token.span;
        let TokenKind::Identifier(name) = token.kind else {
            return Err(Error::parse_at(
                "expected 'target' after 'new.'",
                token_span,
            ));
        };
        if name != NEW_TARGET_PROPERTY_NAME {
            return Err(Error::parse_at(
                "expected 'target' after 'new.'",
                token_span,
            ));
        }
        if !self.allows_new_target() {
            return Err(Error::parse_at(
                "new.target is only valid inside functions",
                new_span,
            ));
        }
        Ok(self.expression_node(new_span, Expr::NewTarget))
    }

    fn update_expr(
        &self,
        op: UpdateOp,
        prefix: bool,
        expr: Expression,
        operator: crate::SourceSpan,
    ) -> Result<Expression> {
        let start = if prefix { operator } else { expr.span() };
        let expr = Self::assignment_target(expr)
            .ok_or_else(|| Error::parse_at("invalid update target", operator))?;
        Ok(self.expression_node(
            start,
            Expr::Update {
                op,
                prefix,
                strict: self.is_strict_mode(),
                expr: Box::new(expr),
            },
        ))
    }

    fn template_literal(&mut self, head: String, start: crate::SourceSpan) -> Result<Expression> {
        let mut quasis = vec![self.static_string(head)?];
        let mut expressions = Vec::new();
        loop {
            expressions.push(self.expression()?);
            let token = self
                .advance()
                .ok_or_else(|| self.parse_error("expected template literal continuation"))?;
            let token_span = token.span;
            match token.kind {
                TokenKind::TemplateMiddle(cooked) => quasis.push(self.static_string(cooked)?),
                TokenKind::TemplateTail(cooked) => {
                    quasis.push(self.static_string(cooked)?);
                    break;
                }
                _ => {
                    return Err(Error::parse_at(
                        "expected '}' to continue template literal",
                        token_span,
                    ));
                }
            }
        }
        Ok(self.expression_node(
            start,
            Expr::TemplateLiteral {
                quasis,
                expressions,
            },
        ))
    }

    fn super_expression(&mut self, start: crate::SourceSpan) -> Result<Expression> {
        if self.check(&TokenKind::LParen) {
            if !self.allow_super_call {
                return Err(Error::parse_at(
                    "super call is only valid inside derived class constructors",
                    start,
                ));
            }
            self.consume(&TokenKind::LParen, "expected '(' after 'super'")?;
            let args = if self.check(&TokenKind::RParen) {
                Vec::new()
            } else {
                self.arguments()?
            };
            self.consume(&TokenKind::RParen, "expected ')' after super arguments")?;
            return Ok(self.expression_node(start, Expr::SuperCall { args }));
        }
        if self.match_kind(&TokenKind::Dot) {
            if !self.allow_super_property {
                return Err(Error::parse_at(
                    "super property access is only valid inside class methods",
                    start,
                ));
            }
            let property = self.consume_identifier("expected property name after 'super.'")?;
            let access = self.static_property_access()?;
            return Ok(self.expression_node(start, Expr::SuperMember { property, access }));
        }
        Err(Error::parse_at(
            "super is only valid in super() calls and super.property access",
            start,
        ))
    }

    fn arguments(&mut self) -> Result<Vec<Expression>> {
        let mut args = Vec::new();
        loop {
            let spread = self.match_kind(&TokenKind::DotDotDot);
            if spread {
                let start = self.previous_span();
                let expression = self.assignment_expression()?;
                args.push(self.expression_node(start, Expr::Spread(Box::new(expression))));
            } else {
                args.push(self.assignment_expression()?);
            }
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.check(&TokenKind::RParen) {
                if spread {
                    return Err(self.parse_error("rest argument must not have a trailing comma"));
                }
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expression> {
        let token = self
            .advance()
            .ok_or_else(|| self.parse_error("expected expression"))?;
        let token_span = token.span;
        let expr = match token.kind {
            TokenKind::Number(value) => {
                Expression::new(Expr::Literal(Value::Number(value)), token_span)
            }
            TokenKind::String(value) => {
                Expression::new(Expr::StringLiteral(self.static_string(value)?), token_span)
            }
            TokenKind::TemplateHead(head) => self.template_literal(head, token_span)?,
            TokenKind::RegExp { pattern, flags } => Expression::new(
                Expr::RegExpLiteral {
                    pattern: self.static_string(pattern)?,
                    flags: self.static_string(flags)?,
                },
                token_span,
            ),
            TokenKind::True => Expression::new(Expr::Literal(Value::Bool(true)), token_span),
            TokenKind::False => Expression::new(Expr::Literal(Value::Bool(false)), token_span),
            TokenKind::Null => Expression::new(Expr::Literal(Value::Null), token_span),
            TokenKind::Undefined => Expression::new(Expr::Literal(Value::Undefined), token_span),
            TokenKind::This => Expression::new(Expr::This, token_span),
            TokenKind::Identifier(name) if name == SUPER_IDENTIFIER_NAME => {
                return Err(Error::parse_at(
                    "super is only valid inside class methods",
                    token_span,
                ));
            }
            TokenKind::Super => self.super_expression(token_span)?,
            TokenKind::Identifier(name) => {
                if self.class_arguments_are_restricted() && name == "arguments" {
                    return Err(Error::parse_at(
                        "arguments is not allowed in a class field or static block",
                        token_span,
                    ));
                }
                if self.yield_identifier_is_reserved() && name == super::YIELD_IDENTIFIER_NAME {
                    return Err(Error::parse_at(
                        "yield is not a valid identifier reference",
                        token_span,
                    ));
                }
                self.validate_strict_identifier_reference(&name)?;
                Expression::new(
                    Expr::Identifier(self.static_binding_name(name)?),
                    token_span,
                )
            }
            TokenKind::Await if !self.await_identifier_is_reserved() => Expression::new(
                Expr::Identifier(self.contextual_await_binding(token_span.start())?),
                token_span,
            ),
            TokenKind::Function => {
                let kind = if self.match_kind(&TokenKind::Star) {
                    FunctionKind::Generator
                } else {
                    FunctionKind::Ordinary
                };
                self.function_expression(kind)?
            }
            TokenKind::Class => self.class_expression()?,
            TokenKind::Async => {
                if self.peek_kind_is_no_line_terminator(0, &TokenKind::Function) {
                    self.consume(&TokenKind::Function, "expected 'function' after 'async'")?;
                    let kind = if self.match_kind(&TokenKind::Star) {
                        FunctionKind::AsyncGenerator
                    } else {
                        FunctionKind::Async
                    };
                    self.function_expression(kind)?
                } else {
                    Expression::new(
                        Expr::Identifier(self.contextual_async_binding(token_span.start())?),
                        token_span,
                    )
                }
            }
            TokenKind::LBrace => self.object_literal()?,
            TokenKind::LBracket => self.array_literal()?,
            TokenKind::LParen => {
                let expr = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')' after expression")?;
                self.expression_node(token_span, Expr::Parenthesized(Box::new(expr)))
            }
            TokenKind::PrivateName(name) => {
                return Err(Error::parse_at(
                    format!("private name '{name}' is only valid in member access or 'in' checks"),
                    token_span,
                ));
            }
            _ => return Err(Error::parse_at("expected expression", token_span)),
        };
        Ok(expr)
    }

    pub(super) fn arrow_function(&mut self) -> Result<Option<Expression>> {
        let Some(signature) = self.arrow_signature() else {
            return Ok(None);
        };
        let start = self.current_span();
        let inherited_strict = self.is_strict_mode();
        let inherited_await_reserved = self.await_identifier_is_reserved();
        let inherited_yield_reserved = self.yield_identifier_is_reserved();
        if signature.is_async {
            self.consume(
                &TokenKind::Async,
                "expected 'async' before async arrow function",
            )?;
        }
        let parameters = match signature.parameters {
            ArrowParameters::Single => {
                let parameter = self.with_await_context(
                    false,
                    signature.is_async || inherited_await_reserved,
                    |parser| {
                        parser.with_yield_expression(false, |parser| {
                            parser.with_yield_identifier_reserved(
                                inherited_yield_reserved,
                                |parser| {
                                    parser.consume_binding_identifier(
                                        "expected arrow function parameter",
                                    )
                                },
                            )
                        })
                    },
                )?;
                super::function::ParsedParameters {
                    params: vec![FunctionParam::new(parameter, None)],
                    pattern_prologue: Vec::new(),
                    is_simple: true,
                }
            }
            ArrowParameters::Parenthesized => {
                self.consume(&TokenKind::LParen, "expected '(' before arrow parameters")?;
                let parameters = self.with_await_context(
                    false,
                    signature.is_async || inherited_await_reserved,
                    |parser| {
                        parser.with_yield_expression(false, |parser| {
                            parser.with_yield_identifier_reserved(
                                inherited_yield_reserved,
                                Self::function_parameters,
                            )
                        })
                    },
                )?;
                self.consume(&TokenKind::RParen, "expected ')' after arrow parameters")?;
                parameters
            }
        };
        self.reject_duplicate_parameters(&parameters.params)?;
        self.consume(&TokenKind::Arrow, "expected '=>' after arrow parameters")?;
        let body = self.arrow_body(inherited_strict, signature.is_async)?;
        self.validate_function_parameters(
            &parameters.params,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        let id = self.static_function()?;
        let (params, statements, parameter_prologue_count) =
            parameters.apply_prologue(body.statements);
        Ok(Some(self.expression_node(
            start,
            Expr::ArrowFunction {
                id,
                params: params.into(),
                body: statements.into(),
                parameter_prologue_count,
                kind: if signature.is_async {
                    FunctionKind::Async
                } else {
                    FunctionKind::Ordinary
                },
            },
        )))
    }

    fn arrow_body(
        &mut self,
        inherited_strict: bool,
        is_async: bool,
    ) -> Result<super::ParsedFunctionBody> {
        self.with_await_context(is_async, is_async, |parser| {
            parser.with_yield_expression(false, |parser| {
                if parser.match_kind(&TokenKind::LBrace) {
                    return parser.function_body(inherited_strict);
                }
                let value = parser.assignment()?;
                let span = value.span();
                Ok(super::ParsedFunctionBody {
                    statements: vec![Statement::new(Stmt::Return(Some(value)), span)],
                    contains_use_strict: false,
                })
            })
        })
    }

    fn arrow_signature(&self) -> Option<ArrowSignature> {
        match self.peek_kind(0)? {
            TokenKind::Identifier(_) | TokenKind::Async
                if self.peek_kind_is_no_line_terminator(1, &TokenKind::Arrow) =>
            {
                Some(ArrowSignature {
                    is_async: false,
                    parameters: ArrowParameters::Single,
                })
            }
            TokenKind::LParen if self.parenthesized_arrow_end(0).is_some() => {
                Some(ArrowSignature {
                    is_async: false,
                    parameters: ArrowParameters::Parenthesized,
                })
            }
            TokenKind::Async => self.async_arrow_signature(),
            _ => None,
        }
    }

    fn async_arrow_signature(&self) -> Option<ArrowSignature> {
        match self.peek_kind(1)? {
            _ if !self.peek_has_line_terminator_before(1)
                && self.peek_is_identifier_name(1)
                && self.peek_kind_is_no_line_terminator(2, &TokenKind::Arrow) =>
            {
                Some(ArrowSignature {
                    is_async: true,
                    parameters: ArrowParameters::Single,
                })
            }
            TokenKind::LParen
                if !self.peek_has_line_terminator_before(1)
                    && self.parenthesized_arrow_end(1).is_some() =>
            {
                Some(ArrowSignature {
                    is_async: true,
                    parameters: ArrowParameters::Parenthesized,
                })
            }
            _ => None,
        }
    }

    fn parenthesized_arrow_end(&self, lparen_offset: usize) -> Option<usize> {
        if !self.peek_kind_is(lparen_offset, &TokenKind::LParen) {
            return None;
        }
        let mut offset = lparen_offset;
        let mut depth = 0usize;
        loop {
            let kind = self.peek_kind(offset)?;
            match kind {
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                    depth = depth.checked_add(1)?;
                }
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        if !self.peek_kind_is(offset, &TokenKind::RParen) {
                            return None;
                        }
                        let arrow = offset.checked_add(1)?;
                        return self
                            .peek_kind_is_no_line_terminator(arrow, &TokenKind::Arrow)
                            .then_some(arrow);
                    }
                }
                TokenKind::Eof => return None,
                _ => {}
            }
            offset = offset.checked_add(1)?;
        }
    }
}
