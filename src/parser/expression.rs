use crate::{
    ast::{
        Expr, Expression, FunctionKind, FunctionParam, ImportPhase, Statement, Stmt, UnaryOp,
        UpdateOp,
    },
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::{Parser, SUPER_IDENTIFIER_NAME};

const NEW_TARGET_PROPERTY_NAME: &str = "target";
const IMPORT_DEFER_PROPERTY_NAME: &str = "defer";
const IMPORT_META_PROPERTY_NAME: &str = "meta";
const IMPORT_SOURCE_PROPERTY_NAME: &str = "source";

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
        if self.await_expression_is_allowed()
            && !self.await_starts_identifier_assignment()
            && self.match_kind(&TokenKind::Await)
        {
            if self.is_module_goal() && self.function_body_depth == 0 {
                self.top_level_await = true;
            }
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
                    strict: self.is_strict_mode(),
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
                    strict: self.is_strict_mode(),
                    expr: Box::new(expr),
                },
            ));
        }
        if self.match_kind(&TokenKind::Delete) {
            return self.delete_unary(start);
        }
        if self.match_kind(&TokenKind::Bang) {
            let expr = self.unary()?;
            return Ok(self.expression_node(
                start,
                Expr::Unary {
                    op: UnaryOp::Not,
                    strict: self.is_strict_mode(),
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
                    strict: self.is_strict_mode(),
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
                    strict: self.is_strict_mode(),
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
                    strict: self.is_strict_mode(),
                    expr: Box::new(expr),
                },
            ));
        }
        self.call()
    }

    fn delete_unary(&mut self, start: crate::SourceSpan) -> Result<Expression> {
        let expr = self.unary()?;
        Self::reject_private_delete_target(&expr)?;
        if self.is_strict_mode() && Self::is_identifier_reference(&expr) {
            return Err(Error::parse_at(
                "delete of an unqualified identifier is not allowed in strict mode",
                expr.span(),
            ));
        }
        Ok(self.expression_node(
            start,
            Expr::Unary {
                op: UnaryOp::Delete,
                strict: self.is_strict_mode(),
                expr: Box::new(expr),
            },
        ))
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
        if !self.peek_has_line_terminator_before(0) && self.match_kind(&TokenKind::PlusPlus) {
            return self.update_expr(UpdateOp::Increment, false, expr, self.previous_span());
        }
        if !self.peek_has_line_terminator_before(0) && self.match_kind(&TokenKind::MinusMinus) {
            return self.update_expr(UpdateOp::Decrement, false, expr, self.previous_span());
        }
        Ok(expr)
    }

    fn new_expr(&mut self) -> Result<Expression> {
        let new_span = self.previous_span();
        if self.match_kind(&TokenKind::Dot) {
            let expr = self.new_target_expr(new_span)?;
            return self.call_suffix(expr);
        }
        if self.match_kind(&TokenKind::Import) {
            return Err(Error::parse_at(
                "import call cannot be used as a constructor",
                new_span,
            ));
        }
        let constructor = self.primary()?;
        let constructor = self.member_suffix(constructor)?;
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
        let token = self.advance_token("expected 'target' after 'new.'")?;
        let token_span = token.span;
        if !token.is_unescaped_identifier_named(NEW_TARGET_PROPERTY_NAME) {
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
        let expr = self
            .assignment_target(expr)
            .ok_or_else(|| Error::parse_at("invalid update target", operator))?;
        self.validate_assignment_target(&expr)?;
        if matches!(expr.kind(), Expr::Call { .. }) {
            return Ok(self.expression_node(
                start,
                Expr::WebCompatCallAssignment {
                    target: Box::new(expr),
                    discarded: None,
                },
            ));
        }
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

    fn template_literal(
        &mut self,
        head: crate::lexer::TemplatePart,
        start: crate::SourceSpan,
    ) -> Result<Expression> {
        let (quasis, expressions) = self.template_parts(head)?;
        Ok(self.expression_node(
            start,
            Expr::TemplateLiteral {
                quasis,
                expressions,
            },
        ))
    }

    fn template_parts(
        &mut self,
        head: crate::lexer::TemplatePart,
    ) -> Result<(Vec<crate::ast::TemplateElement>, Vec<Expression>)> {
        let mut quasis = vec![self.template_element(head)?];
        let mut expressions = Vec::new();
        loop {
            expressions.push(self.expression()?);
            let token = self.advance_token("expected template literal continuation")?;
            let token_span = token.span;
            match token.kind {
                TokenKind::TemplateMiddle(part) => quasis.push(self.template_element(part)?),
                TokenKind::TemplateTail(part) => {
                    quasis.push(self.template_element(part)?);
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
        Ok((quasis, expressions))
    }

    fn template_element(
        &mut self,
        part: crate::lexer::TemplatePart,
    ) -> Result<crate::ast::TemplateElement> {
        Ok(crate::ast::TemplateElement {
            cooked: self.static_string_shared(part.cooked)?,
            raw: self.static_string_shared(part.raw)?,
        })
    }

    fn string_literal(
        &mut self,
        value: crate::lexer::StringToken,
        span: crate::SourceSpan,
    ) -> Result<Expression> {
        Ok(Expression::new(
            Expr::StringLiteral {
                value: self.static_string_shared(value.cooked)?,
                escape_free: value.escape_free,
            },
            span,
        ))
    }

    fn no_substitution_template(
        &mut self,
        part: crate::lexer::TemplatePart,
        span: crate::SourceSpan,
    ) -> Result<Expression> {
        let quasi = self.template_element(part)?;
        Ok(Expression::new(
            Expr::TemplateLiteral {
                quasis: vec![quasi],
                expressions: Vec::new(),
            },
            span,
        ))
    }

    fn advance_primary_token(&mut self) -> Result<crate::lexer::Token> {
        let token = self
            .advance_regexp()
            .ok_or_else(|| self.parse_error("expected expression"))?;
        if token.identifier_escaped && super::property_name::is_reserved_word_token(&token.kind) {
            return Err(Error::parse_at("escaped reserved word", token.span));
        }
        Ok(token)
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
            let property = self.consume_property_name("expected property name after 'super.'")?;
            let access = self.static_property_access()?;
            return Ok(self.expression_node(start, Expr::SuperMember { property, access }));
        }
        if self.match_kind(&TokenKind::LBracket) {
            if !self.allow_super_property {
                return Err(Error::parse_at(
                    "super property access is only valid inside methods",
                    start,
                ));
            }
            let property = self.expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after super property expression",
            )?;
            let access = self.static_property_access()?;
            return Ok(self.expression_node(
                start,
                Expr::SuperComputedMember {
                    property: Box::new(property),
                    access,
                },
            ));
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

    fn dynamic_import(&mut self, start: crate::SourceSpan) -> Result<Expression> {
        let phase = if self.match_kind(&TokenKind::Dot) {
            let token = self.advance_token("expected import phase after 'import.'")?;
            if token.is_unescaped_identifier_named(IMPORT_META_PROPERTY_NAME) {
                if !self.is_module_goal() {
                    return Err(Error::parse_at(
                        "import.meta is only valid in modules",
                        token.span,
                    ));
                }
                return Ok(self.expression_node(start, Expr::ImportMeta));
            } else if token.is_unescaped_identifier_named(IMPORT_SOURCE_PROPERTY_NAME) {
                ImportPhase::Source
            } else if token.is_unescaped_identifier_named(IMPORT_DEFER_PROPERTY_NAME) {
                ImportPhase::Defer
            } else {
                return Err(Error::parse_at("invalid import phase", token.span));
            }
        } else {
            ImportPhase::Evaluation
        };
        self.consume(&TokenKind::LParen, "expected '(' after import")?;
        if self.check(&TokenKind::RParen) || self.check(&TokenKind::DotDotDot) {
            return Err(self.parse_error("import call requires one specifier expression"));
        }
        let specifier = self.assignment_expression()?;
        let options = if self.match_kind(&TokenKind::Comma) && !self.check(&TokenKind::RParen) {
            if self.check(&TokenKind::DotDotDot) {
                return Err(self.parse_error("import call does not accept spread arguments"));
            }
            let options = self.assignment_expression()?;
            if self.match_kind(&TokenKind::Comma) && !self.check(&TokenKind::RParen) {
                return Err(self.parse_error("import call accepts at most two arguments"));
            }
            Some(Box::new(options))
        } else {
            None
        };
        self.consume(&TokenKind::RParen, "expected ')' after import arguments")?;
        Ok(self.expression_node(
            start,
            Expr::DynamicImport {
                phase,
                specifier: Box::new(specifier),
                options,
            },
        ))
    }

    fn primary(&mut self) -> Result<Expression> {
        let token = self.advance_primary_token()?;
        let token_span = token.span;
        Ok(match token.kind {
            TokenKind::LexicalError(error) => return Err(*error),
            TokenKind::Number(value) => {
                Expression::new(Expr::Literal(Value::Number(value)), token_span)
            }
            TokenKind::BigInt(value) => {
                Expression::new(Expr::Literal(Value::BigInt(value)), token_span)
            }
            TokenKind::String(value) => self.string_literal(value, token_span)?,
            TokenKind::NoSubstitutionTemplate(part) => {
                self.no_substitution_template(part, token_span)?
            }
            TokenKind::TemplateHead(head) => self.template_literal(head, token_span)?,
            TokenKind::RegExp { pattern, flags } => Expression::new(
                Expr::RegExpLiteral {
                    pattern: self.static_string(pattern.encode_utf16().collect())?,
                    flags: self.static_string(flags.encode_utf16().collect())?,
                },
                token_span,
            ),
            TokenKind::True => Expression::new(Expr::Literal(Value::Bool(true)), token_span),
            TokenKind::False => Expression::new(Expr::Literal(Value::Bool(false)), token_span),
            TokenKind::Null => Expression::new(Expr::Literal(Value::Null), token_span),
            TokenKind::This => Expression::new(Expr::This, token_span),
            TokenKind::Identifier(name) if name.as_ref() == SUPER_IDENTIFIER_NAME => {
                return Err(Error::parse_at(
                    "super is only valid inside class methods",
                    token_span,
                ));
            }
            TokenKind::Super => self.super_expression(token_span)?,
            TokenKind::Import => self.dynamic_import(token_span)?,
            TokenKind::Identifier(name) => {
                if self.class_arguments_are_restricted() && name.as_ref() == "arguments" {
                    return Err(Error::parse_at(
                        "arguments is not allowed in a class field or static block",
                        token_span,
                    ));
                }
                if self.yield_identifier_is_reserved()
                    && name.as_ref() == super::YIELD_IDENTIFIER_NAME
                {
                    return Err(Error::parse_at(
                        "yield is not a valid identifier reference",
                        token_span,
                    ));
                }
                self.validate_strict_identifier_reference(name.as_ref())?;
                self.note_arguments_reference(name.as_ref());
                Expression::new(
                    Expr::Identifier(self.static_binding_name_shared(name)?),
                    token_span,
                )
            }
            TokenKind::Await if !self.await_identifier_is_reserved() => Expression::new(
                Expr::Identifier(self.contextual_await_binding(token_span.start())?),
                token_span,
            ),
            TokenKind::Let => self.contextual_let(token_span)?,
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
        })
    }

    fn contextual_let(&mut self, span: crate::SourceSpan) -> Result<Expression> {
        if self.is_strict_mode() {
            return Err(Error::parse_at("expected expression", span));
        }
        let name = self.static_name_borrowed_at("let", span.start())?;
        Ok(Expression::new(
            Expr::Identifier(self.static_binding(name)?),
            span,
        ))
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
                    params: vec![FunctionParam::new(parameter.clone(), None)],
                    bound_names: vec![parameter],
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
        self.reject_duplicate_parameters(&parameters.bound_names)?;
        self.consume(&TokenKind::Arrow, "expected '=>' after arrow parameters")?;
        let body = self.arrow_body(inherited_strict, signature.is_async)?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        let id = self.static_function()?;
        let strict = inherited_strict || body.contains_use_strict;
        let params = parameters.into_params();
        let statements = body.statements;
        Ok(Some(self.expression_node(
            start,
            Expr::ArrowFunction {
                id,
                params: params.into(),
                body: statements.into(),
                kind: if signature.is_async {
                    FunctionKind::Async
                } else {
                    FunctionKind::Ordinary
                },
                strict,
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
                parser.function_body_depth = parser
                    .function_body_depth
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("function body nesting overflowed"))?;
                let value = parser.assignment();
                parser.function_body_depth = parser.function_body_depth.saturating_sub(1);
                let value = value?;
                let span = value.span();
                Ok(super::ParsedFunctionBody {
                    statements: vec![Statement::new(Stmt::Return(Some(value)), span)],
                    contains_use_strict: false,
                })
            })
        })
    }

    fn arrow_signature(&mut self) -> Option<ArrowSignature> {
        let identifier = matches!(
            self.peek_kind(0)?,
            TokenKind::Identifier(_) | TokenKind::Async
        );
        if identifier && self.peek_kind_is_no_line_terminator(1, &TokenKind::Arrow) {
            return Some(ArrowSignature {
                is_async: false,
                parameters: ArrowParameters::Single,
            });
        }
        let parenthesized = matches!(self.peek_kind(0)?, TokenKind::LParen);
        if parenthesized && self.parenthesized_arrow_end(0).is_some() {
            return Some(ArrowSignature {
                is_async: false,
                parameters: ArrowParameters::Parenthesized,
            });
        }
        if matches!(self.peek_kind(0)?, TokenKind::Async) {
            self.async_arrow_signature()
        } else {
            None
        }
    }

    fn async_arrow_signature(&mut self) -> Option<ArrowSignature> {
        let no_line_terminator = !self.peek_has_line_terminator_before(1);
        if no_line_terminator
            && self.peek_is_identifier_name(1)
            && self.peek_kind_is_no_line_terminator(2, &TokenKind::Arrow)
        {
            return Some(ArrowSignature {
                is_async: true,
                parameters: ArrowParameters::Single,
            });
        }
        let parenthesized = matches!(self.peek_kind(1)?, TokenKind::LParen);
        (no_line_terminator && parenthesized && self.parenthesized_arrow_end(1).is_some())
            .then_some(ArrowSignature {
                is_async: true,
                parameters: ArrowParameters::Parenthesized,
            })
    }

    fn parenthesized_arrow_end(&mut self, lparen_offset: usize) -> Option<usize> {
        let closing = self.balanced_closing_offset(lparen_offset)?;
        let arrow = closing.checked_add(1)?;
        self.peek_kind_is_no_line_terminator(arrow, &TokenKind::Arrow)
            .then_some(arrow)
    }
}
