use crate::{
    ast::{ArrayBindingElement, BindingPattern, BindingPropertyKey, ObjectBindingProperty},
    error::{Error, Result},
    lexer::TokenKind,
};

use super::{Parser, literal::ObjectPropertyName};

impl Parser {
    pub(super) fn next_is_binding_pattern(&self) -> bool {
        self.check(&TokenKind::LBrace) || self.check(&TokenKind::LBracket)
    }

    pub(super) fn binding_pattern(&mut self) -> Result<BindingPattern> {
        self.with_pattern_depth(Self::binding_pattern_inner)
    }

    fn binding_pattern_inner(&mut self) -> Result<BindingPattern> {
        if self.match_kind(&TokenKind::LBrace) {
            return self.object_binding_pattern();
        }
        if self.match_kind(&TokenKind::LBracket) {
            return self.array_binding_pattern();
        }
        Ok(BindingPattern::Identifier(
            self.consume_binding_identifier("expected binding name")?,
        ))
    }

    fn object_binding_pattern(&mut self) -> Result<BindingPattern> {
        let mut properties = Vec::new();
        let mut rest = None;
        loop {
            if self.check(&TokenKind::RBrace) {
                break;
            }
            if self.match_kind(&TokenKind::DotDotDot) {
                rest = Some(self.consume_binding_identifier("expected object rest binding name")?);
                break;
            }
            properties.push(self.object_binding_property()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(
            &TokenKind::RBrace,
            "expected '}' after object binding pattern",
        )?;
        Ok(BindingPattern::Object { properties, rest })
    }

    fn object_binding_property(&mut self) -> Result<ObjectBindingProperty> {
        let name = self.object_property_key()?;
        let (key, shorthand_name) = match name {
            ObjectPropertyName::Static {
                key,
                shorthand_name,
            } => (BindingPropertyKey::Static(key), shorthand_name),
            ObjectPropertyName::Computed(expr) => (BindingPropertyKey::Computed(expr), None),
        };

        if self.match_kind(&TokenKind::Colon) {
            let target = self.binding_pattern()?;
            let default = self.optional_binding_default()?;
            return Ok(ObjectBindingProperty {
                key,
                target,
                default,
            });
        }

        let Some(shorthand_name) = shorthand_name else {
            return Err(self.parse_error("expected ':' after object binding property name"));
        };
        let binding = self.static_binding(shorthand_name)?;
        let default = self.optional_binding_default()?;
        Ok(ObjectBindingProperty {
            key,
            target: BindingPattern::Identifier(binding),
            default,
        })
    }

    fn array_binding_pattern(&mut self) -> Result<BindingPattern> {
        let mut elements = Vec::new();
        let mut rest = None;
        loop {
            if self.check(&TokenKind::RBracket) {
                break;
            }
            if self.match_kind(&TokenKind::Comma) {
                elements.push(None);
                continue;
            }
            if self.match_kind(&TokenKind::DotDotDot) {
                rest = Some(Box::new(self.binding_pattern()?));
                break;
            }
            let target = self.binding_pattern()?;
            let default = self.optional_binding_default()?;
            elements.push(Some(ArrayBindingElement { target, default }));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(
            &TokenKind::RBracket,
            "expected ']' after array binding pattern",
        )?;
        Ok(BindingPattern::Array { elements, rest })
    }

    fn optional_binding_default(&mut self) -> Result<Option<crate::ast::Expr>> {
        if self.match_kind(&TokenKind::Equal) {
            return Ok(Some(self.expression()?));
        }
        Ok(None)
    }

    fn with_pattern_depth(
        &mut self,
        parse: impl FnOnce(&mut Self) -> Result<BindingPattern>,
    ) -> Result<BindingPattern> {
        self.expression_depth = self
            .expression_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("binding pattern nesting overflowed"))?;
        if self.expression_depth > self.limits.max_expression_depth {
            self.expression_depth = self.expression_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "binding pattern nesting exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = parse(self);
        self.expression_depth = self.expression_depth.saturating_sub(1);
        result
    }
}
