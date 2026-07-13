use crate::{
    ast::{Expr, Expression, FunctionKind, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::{Parser, property_name::keyword_property_name};

/// Placeholder key for spread object-literal entries; the runtime ignores it.
const SPREAD_PROPERTY_KEY: &str = "...";
const GETTER_KEYWORD_NAME: &str = "get";
const SETTER_KEYWORD_NAME: &str = "set";

pub(super) enum ObjectPropertyName {
    Static {
        key: crate::ast::StaticName,
        shorthand_name: Option<crate::ast::StaticName>,
    },
    Computed(Expression),
}

impl Parser {
    pub(super) fn object_literal(&mut self) -> Result<Expression> {
        let start = self.previous_span();
        let mut properties = Vec::new();
        if self.match_kind(&TokenKind::RBrace) {
            return Ok(self.expression_node(start, Expr::Object(properties)));
        }

        loop {
            properties.push(self.object_literal_property()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBrace) {
                return Ok(self.expression_node(start, Expr::Object(properties)));
            }
        }

        self.consume(&TokenKind::RBrace, "expected '}' after object literal")?;
        Ok(self.expression_node(start, Expr::Object(properties)))
    }

    fn object_literal_property(&mut self) -> Result<ObjectProperty> {
        let start = self.current_span();
        if self.match_kind(&TokenKind::DotDotDot) {
            let value = self.assignment_expression()?;
            let key = ObjectPropertyKey::Static(self.static_name(SPREAD_PROPERTY_KEY.to_owned())?);
            return Ok(ObjectProperty {
                key,
                kind: ObjectPropertyKind::Spread,
                value,
            });
        }
        if self.async_object_method_start() {
            self.consume(
                &TokenKind::Async,
                "expected 'async' before async object method",
            )?;
            let kind = if self.match_kind(&TokenKind::Star) {
                FunctionKind::AsyncGenerator
            } else {
                FunctionKind::Async
            };
            let name = self.object_property_key()?;
            return self.object_method_property(name, kind, start);
        }
        if self.match_kind(&TokenKind::Star) {
            let name = self.object_property_key()?;
            return self.object_method_property(name, FunctionKind::Generator, start);
        }
        if let Some(kind) = self.object_accessor_start() {
            let keyword = self.advance_token("expected accessor keyword")?;
            let name = self.object_property_key()?;
            return self.object_accessor_property(name, kind, keyword.span);
        }
        let name = self.object_property_key()?;
        if self.match_kind(&TokenKind::Colon) {
            let value = self.assignment_expression()?;
            return Ok(ObjectProperty {
                key: name.into_key(),
                kind: ObjectPropertyKind::Init,
                value,
            });
        }
        if self.match_kind(&TokenKind::LParen) {
            return self.object_method_property_after_lparen(name, FunctionKind::Ordinary, start);
        }
        if let ObjectPropertyName::Static {
            key,
            shorthand_name: Some(binding),
        } = name
        {
            self.note_arguments_reference(binding.as_str());
            let binding = self.static_binding(binding)?;
            return Ok(ObjectProperty {
                key: ObjectPropertyKey::Static(key),
                kind: ObjectPropertyKind::Init,
                value: self.expression_node(start, Expr::Identifier(binding)),
            });
        }
        Err(self.parse_error("expected ':' after object property name"))
    }

    /// Detects a `get name` / `set name` accessor definition. A `get`/`set`
    /// identifier followed by anything that can start a property name is an
    /// accessor; otherwise it is an ordinary key (`{get: 1}`, `{get() {}}`,
    /// `{get}`), which falls through to the regular property paths.
    fn object_accessor_start(&mut self) -> Option<ObjectPropertyKind> {
        let kind = self.peek().and_then(|token| {
            if token.is_unescaped_identifier_named(GETTER_KEYWORD_NAME) {
                Some(ObjectPropertyKind::Get)
            } else if token.is_unescaped_identifier_named(SETTER_KEYWORD_NAME) {
                Some(ObjectPropertyKind::Set)
            } else {
                None
            }
        })?;
        self.peek_kind(1)
            .is_some_and(is_object_property_name_start)
            .then_some(kind)
    }

    fn object_accessor_property(
        &mut self,
        name: ObjectPropertyName,
        kind: ObjectPropertyKind,
        start: crate::SourceSpan,
    ) -> Result<ObjectProperty> {
        self.consume(&TokenKind::LParen, "expected '(' after accessor name")?;
        let inherited_strict = self.is_strict_mode();
        let ((parameters, body), uses_arguments) =
            self.with_function_arguments_context(|parser| {
                let parameters =
                    parser.with_await_context(false, false, Self::function_parameters)?;
                parser.consume(&TokenKind::RParen, "expected ')' after accessor parameters")?;
                Self::validate_object_accessor_parameters(kind, &parameters, start)?;
                parser.consume(&TokenKind::LBrace, "expected '{' before accessor body")?;
                let body = parser.with_new_target_scope(|parser| {
                    parser.with_super_context(true, false, |parser| {
                        parser.with_await_context(false, false, |parser| {
                            parser.function_body(inherited_strict)
                        })
                    })
                })?;
                Ok((parameters, body))
            })?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        let id = self.static_function()?;
        let strict = inherited_strict || body.contains_use_strict;
        let arguments_binding = if uses_arguments {
            Some(self.implicit_arguments_binding()?)
        } else {
            None
        };
        let params = parameters.into_params();
        let statements = body.statements;
        let key = name.into_key();
        let name = match &key {
            ObjectPropertyKey::Static(name) => Some(name.clone()),
            ObjectPropertyKey::Computed(_) => None,
        };
        let value = self.expression_node(
            start,
            Expr::MethodFunction {
                id,
                name,
                arguments_binding,
                params: params.into(),
                body: statements.into(),
                kind: FunctionKind::Ordinary,
                strict,
            },
        );
        Ok(ObjectProperty { key, kind, value })
    }

    fn validate_object_accessor_parameters(
        kind: ObjectPropertyKind,
        parameters: &super::function::ParsedParameters,
        start: crate::SourceSpan,
    ) -> Result<()> {
        match kind {
            ObjectPropertyKind::Get if !parameters.params.is_empty() => {
                Err(Error::parse_at("getter must not declare parameters", start))
            }
            ObjectPropertyKind::Set if parameters.params.len() != 1 => Err(Error::parse_at(
                "setter must declare exactly one parameter",
                start,
            )),
            ObjectPropertyKind::Set
                if parameters.params.first().is_some_and(|param| param.rest) =>
            {
                Err(Error::parse_at(
                    "setter parameter cannot be a rest parameter",
                    start,
                ))
            }
            ObjectPropertyKind::Init
            | ObjectPropertyKind::Get
            | ObjectPropertyKind::Set
            | ObjectPropertyKind::Spread => Ok(()),
        }
    }

    fn async_object_method_start(&mut self) -> bool {
        self.peek_kind_is(0, &TokenKind::Async)
            && !self.peek_has_line_terminator_before(1)
            && (self.peek_kind(1).is_some_and(is_object_property_name_start)
                || (self.peek_kind_is(1, &TokenKind::Star)
                    && self.peek_kind(2).is_some_and(is_object_property_name_start)))
    }

    fn object_method_property(
        &mut self,
        name: ObjectPropertyName,
        kind: FunctionKind,
        start: crate::SourceSpan,
    ) -> Result<ObjectProperty> {
        self.consume(&TokenKind::LParen, "expected '(' after object method name")?;
        self.object_method_property_after_lparen(name, kind, start)
    }

    fn object_method_property_after_lparen(
        &mut self,
        name: ObjectPropertyName,
        kind: FunctionKind,
        start: crate::SourceSpan,
    ) -> Result<ObjectProperty> {
        let inherited_strict = self.is_strict_mode();
        let ((parameters, body), uses_arguments) =
            self.with_function_arguments_context(|parser| {
                let parameters = parser.with_await_context(false, kind.is_async(), |parser| {
                    parser.with_yield_expression(false, |parser| {
                        parser.with_yield_identifier_reserved(
                            kind.is_generator(),
                            Self::function_parameters,
                        )
                    })
                })?;
                parser.reject_duplicate_parameters(&parameters.bound_names)?;
                parser.consume(&TokenKind::RParen, "expected ')' after method parameters")?;
                parser.consume(&TokenKind::LBrace, "expected '{' before method body")?;
                let body = parser.with_new_target_scope(|parser| {
                    parser.with_super_context(true, false, |parser| {
                        parser.with_await_context(kind.is_async(), kind.is_async(), |parser| {
                            parser.with_yield_expression(kind.is_generator(), |parser| {
                                parser.function_body(inherited_strict)
                            })
                        })
                    })
                })?;
                Ok((parameters, body))
            })?;
        self.validate_function_parameters(
            &parameters.bound_names,
            parameters.is_simple,
            inherited_strict,
            body.contains_use_strict,
        )?;
        if kind.is_generator() {
            self.validate_generator_parameter_lexicals(&parameters.params, &body.statements)?;
        }
        let id = self.static_function()?;
        let strict = inherited_strict || body.contains_use_strict;
        let arguments_binding = if uses_arguments {
            Some(self.implicit_arguments_binding()?)
        } else {
            None
        };
        let params = parameters.into_params();
        let statements = body.statements;
        let key = name.into_key();
        let name = match &key {
            ObjectPropertyKey::Static(name) => Some(name.clone()),
            ObjectPropertyKey::Computed(_) => None,
        };
        let value = self.expression_node(
            start,
            Expr::MethodFunction {
                id,
                name,
                arguments_binding,
                params: params.into(),
                body: statements.into(),
                kind,
                strict,
            },
        );
        Ok(ObjectProperty {
            key,
            kind: ObjectPropertyKind::Init,
            value,
        })
    }

    pub(super) fn array_literal(&mut self) -> Result<Expression> {
        let start = self.previous_span();
        let mut elements = Vec::new();
        if self.match_kind(&TokenKind::RBracket) {
            return Ok(self.expression_node(start, Expr::Array(elements)));
        }

        loop {
            if self.match_kind(&TokenKind::DotDotDot) {
                let spread_start = self.previous_span();
                let expression = self.assignment_expression()?;
                elements
                    .push(self.expression_node(spread_start, Expr::Spread(Box::new(expression))));
            } else if self.peek_kind_is(0, &TokenKind::Comma)
                || self.peek_kind_is(0, &TokenKind::RBracket)
            {
                elements.push(Expression::new(Expr::ArrayHole, self.current_span()));
            } else {
                elements.push(self.assignment_expression()?);
            }
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
            if self.match_kind(&TokenKind::RBracket) {
                return Ok(self.expression_node(start, Expr::Array(elements)));
            }
        }

        self.consume(&TokenKind::RBracket, "expected ']' after array literal")?;
        Ok(self.expression_node(start, Expr::Array(elements)))
    }

    pub(super) fn object_property_key(&mut self) -> Result<ObjectPropertyName> {
        if self.match_kind(&TokenKind::LBracket) {
            let expr = self.assignment_expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after computed object property name",
            )?;
            return Ok(ObjectPropertyName::Computed(expr));
        }
        let token = self.advance_token("expected object property name")?;
        let token_span = token.span;
        match token.kind {
            TokenKind::Identifier(name) => {
                let name = self.static_name_shared(name)?;
                Ok(ObjectPropertyName::Static {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::Async => {
                let name = self.borrowed_static_name("async")?;
                Ok(ObjectPropertyName::Static {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::Await if !self.await_identifier_is_reserved() => {
                let name = self.borrowed_static_name("await")?;
                Ok(ObjectPropertyName::Static {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::String(name) => {
                let name = String::from_utf16(&name.cooked).map_err(|_| {
                    Error::parse_at(
                        "object property names containing lone surrogates are not supported yet",
                        token_span,
                    )
                })?;
                Ok(ObjectPropertyName::Static {
                    key: self.static_name(name)?,
                    shorthand_name: None,
                })
            }
            TokenKind::Number(value) => Ok(ObjectPropertyName::Static {
                key: self.static_name(Value::Number(value).to_string())?,
                shorthand_name: None,
            }),
            TokenKind::BigInt(value) => Ok(ObjectPropertyName::Static {
                key: self.static_name(value.to_string())?,
                shorthand_name: None,
            }),
            kind => {
                let Some(name) = keyword_property_name(&kind) else {
                    return Err(Error::parse_at("expected object property name", token_span));
                };
                self.keyword_property_name(name)
            }
        }
    }

    fn keyword_property_name(&mut self, name: &str) -> Result<ObjectPropertyName> {
        Ok(ObjectPropertyName::Static {
            key: self.borrowed_static_name(name)?,
            shorthand_name: None,
        })
    }
}

impl ObjectPropertyName {
    fn into_key(self) -> ObjectPropertyKey {
        match self {
            Self::Static { key, .. } => ObjectPropertyKey::Static(key),
            Self::Computed(expr) => ObjectPropertyKey::Computed(Box::new(expr)),
        }
    }
}

pub(super) const fn is_object_property_name_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::String(_)
            | TokenKind::Number(_)
            | TokenKind::BigInt(_)
            | TokenKind::LBracket
    ) || keyword_property_name(kind).is_some()
}
