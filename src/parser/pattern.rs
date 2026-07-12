use crate::{
    ast::{
        ArrayAssignmentElement, ArrayBindingElement, AssignmentPattern, BindingPattern,
        ObjectAssignmentProperty, ObjectBindingProperty, PatternPropertyKey,
    },
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

    pub(super) fn assignment_pattern(&mut self) -> Result<AssignmentPattern> {
        self.with_pattern_depth(Self::assignment_pattern_inner)
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

    fn assignment_pattern_inner(&mut self) -> Result<AssignmentPattern> {
        if self.literal_starts_assignment_target() {
            return self.assignment_pattern_target();
        }
        if self.match_kind(&TokenKind::LBrace) {
            return self.object_assignment_pattern();
        }
        if self.match_kind(&TokenKind::LBracket) {
            return self.array_assignment_pattern();
        }
        self.assignment_pattern_target()
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
            } => (PatternPropertyKey::Static(key), shorthand_name),
            ObjectPropertyName::Computed(expr) => (PatternPropertyKey::Computed(expr), None),
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

    fn object_assignment_pattern(&mut self) -> Result<AssignmentPattern> {
        let mut properties = Vec::new();
        let mut rest = None;
        loop {
            if self.check(&TokenKind::RBrace) {
                break;
            }
            if self.match_kind(&TokenKind::DotDotDot) {
                let AssignmentPattern::Target(target) = self.assignment_pattern_target()? else {
                    return Err(self.parse_error("object rest must be an assignment target"));
                };
                rest = Some(Box::new(target));
                break;
            }
            properties.push(self.object_assignment_property()?);
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(
            &TokenKind::RBrace,
            "expected '}' after object assignment pattern",
        )?;
        Ok(AssignmentPattern::Object { properties, rest })
    }

    fn object_assignment_property(&mut self) -> Result<ObjectAssignmentProperty> {
        let name = self.object_property_key()?;
        let (key, shorthand_name) = match name {
            ObjectPropertyName::Static {
                key,
                shorthand_name,
            } => (PatternPropertyKey::Static(key), shorthand_name),
            ObjectPropertyName::Computed(expr) => (PatternPropertyKey::Computed(expr), None),
        };

        if self.match_kind(&TokenKind::Colon) {
            let target = self.assignment_pattern()?;
            let default = self.optional_assignment_default()?;
            return Ok(ObjectAssignmentProperty {
                key,
                target,
                default,
            });
        }

        let Some(shorthand_name) = shorthand_name else {
            return Err(self.parse_error("expected ':' after object assignment property name"));
        };
        self.validate_assignment_identifier(shorthand_name.as_str())?;
        let binding = self.static_binding(shorthand_name)?;
        let span = self.previous_span();
        let target = AssignmentPattern::Target(
            self.expression_node(span, crate::ast::Expr::Identifier(binding)),
        );
        let default = self.optional_assignment_default()?;
        Ok(ObjectAssignmentProperty {
            key,
            target,
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

    fn array_assignment_pattern(&mut self) -> Result<AssignmentPattern> {
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
                rest = Some(Box::new(self.assignment_pattern()?));
                break;
            }
            let target = self.assignment_pattern()?;
            let default = self.optional_assignment_default()?;
            elements.push(Some(ArrayAssignmentElement { target, default }));
            if !self.match_kind(&TokenKind::Comma) {
                break;
            }
        }
        self.consume(
            &TokenKind::RBracket,
            "expected ']' after array assignment pattern",
        )?;
        Ok(AssignmentPattern::Array { elements, rest })
    }

    fn assignment_pattern_target(&mut self) -> Result<AssignmentPattern> {
        let target = self.conditional()?;
        let Some(target) = Self::assignment_target(target) else {
            return Err(self.parse_error("invalid destructuring assignment target"));
        };
        self.validate_assignment_target(&target)?;
        Ok(AssignmentPattern::Target(target))
    }

    fn optional_binding_default(&mut self) -> Result<Option<crate::ast::Expression>> {
        if self.match_kind(&TokenKind::Equal) {
            return Ok(Some(self.assignment_expression()?));
        }
        Ok(None)
    }

    fn optional_assignment_default(&mut self) -> Result<Option<crate::ast::Expression>> {
        if self.match_kind(&TokenKind::Equal) {
            return Ok(Some(self.assignment_expression()?));
        }
        Ok(None)
    }

    fn with_pattern_depth<T>(&mut self, parse: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
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
