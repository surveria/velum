use crate::{
    ast::{Expr, Expression, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind},
    error::{Error, Result},
    lexer::TokenKind,
    value::Value,
};

use super::{Parser, expression::keyword_property_name};

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
            let value = self.expression()?;
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
            let name = self.object_property_key()?;
            return self.object_method_property(name, true, start);
        }
        if let Some(kind) = self.object_accessor_start() {
            let keyword = self
                .advance()
                .ok_or_else(|| self.parse_error("expected accessor keyword"))?;
            let name = self.object_property_key()?;
            return self.object_accessor_property(name, kind, keyword.span);
        }
        let name = self.object_property_key()?;
        if self.match_kind(&TokenKind::Colon) {
            let value = self.expression()?;
            return Ok(ObjectProperty {
                key: name.into_key(),
                kind: ObjectPropertyKind::Init,
                value,
            });
        }
        if self.match_kind(&TokenKind::LParen) {
            return self.object_method_property_after_lparen(name, false, start);
        }
        if let ObjectPropertyName::Static {
            key,
            shorthand_name: Some(binding),
        } = name
        {
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
    fn object_accessor_start(&self) -> Option<ObjectPropertyKind> {
        let TokenKind::Identifier(name) = self.peek_kind(0)? else {
            return None;
        };
        let kind = match name.as_str() {
            GETTER_KEYWORD_NAME => ObjectPropertyKind::Get,
            SETTER_KEYWORD_NAME => ObjectPropertyKind::Set,
            _ => return None,
        };
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
        let parameters = self.function_parameters()?;
        self.consume(&TokenKind::RParen, "expected ')' after accessor parameters")?;
        match kind {
            ObjectPropertyKind::Get if !parameters.params.is_empty() => {
                return Err(Error::parse_at("getter must not declare parameters", start));
            }
            ObjectPropertyKind::Set if parameters.params.len() != 1 => {
                return Err(Error::parse_at(
                    "setter must declare exactly one parameter",
                    start,
                ));
            }
            ObjectPropertyKind::Set
                if parameters.params.first().is_some_and(|param| param.rest) =>
            {
                return Err(Error::parse_at(
                    "setter parameter cannot be a rest parameter",
                    start,
                ));
            }
            ObjectPropertyKind::Init
            | ObjectPropertyKind::Get
            | ObjectPropertyKind::Set
            | ObjectPropertyKind::Spread => {}
        }
        self.consume(&TokenKind::LBrace, "expected '{' before accessor body")?;
        let body = self.with_new_target_scope(|parser| {
            parser.with_super_context(false, false, |parser| {
                parser.function_body(inherited_strict)
            })
        })?;
        self.validate_function_parameters(
            &parameters.params,
            inherited_strict,
            body.contains_use_strict,
        )?;
        let id = self.static_function()?;
        let (params, statements) = parameters.apply_prologue(body.statements);
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
                params: params.into(),
                body: statements.into(),
                is_async: false,
            },
        );
        Ok(ObjectProperty { key, kind, value })
    }

    fn async_object_method_start(&self) -> bool {
        self.peek_kind_is(0, &TokenKind::Async)
            && !self.peek_has_line_terminator_before(1)
            && self.peek_kind(1).is_some_and(is_object_property_name_start)
    }

    fn object_method_property(
        &mut self,
        name: ObjectPropertyName,
        is_async: bool,
        start: crate::SourceSpan,
    ) -> Result<ObjectProperty> {
        self.consume(&TokenKind::LParen, "expected '(' after object method name")?;
        self.object_method_property_after_lparen(name, is_async, start)
    }

    fn object_method_property_after_lparen(
        &mut self,
        name: ObjectPropertyName,
        is_async: bool,
        start: crate::SourceSpan,
    ) -> Result<ObjectProperty> {
        let inherited_strict = self.is_strict_mode();
        let parameters = self.function_parameters()?;
        self.consume(&TokenKind::RParen, "expected ')' after method parameters")?;
        self.consume(&TokenKind::LBrace, "expected '{' before method body")?;
        let body = self.with_new_target_scope(|parser| {
            parser.with_super_context(false, false, |parser| {
                parser.function_body(inherited_strict)
            })
        })?;
        self.validate_function_parameters(
            &parameters.params,
            inherited_strict,
            body.contains_use_strict,
        )?;
        let id = self.static_function()?;
        let (params, statements) = parameters.apply_prologue(body.statements);
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
                params: params.into(),
                body: statements.into(),
                is_async,
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
                let expression = self.expression()?;
                elements
                    .push(self.expression_node(spread_start, Expr::Spread(Box::new(expression))));
            } else if self.peek_kind_is(0, &TokenKind::Comma)
                || self.peek_kind_is(0, &TokenKind::RBracket)
            {
                elements.push(Expression::new(Expr::ArrayHole, self.current_span()));
            } else {
                elements.push(self.expression()?);
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
            let expr = self.expression()?;
            self.consume(
                &TokenKind::RBracket,
                "expected ']' after computed object property name",
            )?;
            return Ok(ObjectPropertyName::Computed(expr));
        }
        let token = self
            .advance()
            .ok_or_else(|| self.parse_error("expected object property name"))?;
        let token_span = token.span;
        match token.kind {
            TokenKind::Identifier(name) => {
                let name = self.static_name(name)?;
                Ok(ObjectPropertyName::Static {
                    key: name.clone(),
                    shorthand_name: Some(name),
                })
            }
            TokenKind::String(name) => Ok(ObjectPropertyName::Static {
                key: self.static_name(name)?,
                shorthand_name: None,
            }),
            TokenKind::Number(value) => Ok(ObjectPropertyName::Static {
                key: self.static_name(Value::Number(value).to_string())?,
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

const fn is_object_property_name_start(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier(_)
            | TokenKind::String(_)
            | TokenKind::Number(_)
            | TokenKind::LBracket
    ) || keyword_property_name(kind).is_some()
}
