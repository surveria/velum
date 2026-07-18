use alloc::rc::Rc;

use crate::{
    ast::{
        ArrayAssignmentElement, ArrayBindingElement, AssignmentPattern, BindingPattern, Expr,
        Expression, ObjectAssignmentProperty, ObjectBindingProperty, PatternPropertyKey,
    },
    bytecode::{
        BytecodeBlock, BytecodePattern, BytecodePatternKey, BytecodePatternProperty,
        BytecodePatternTarget,
    },
    error::Result,
};

use super::BytecodeCompiler;

impl BytecodeCompiler<'_> {
    pub(super) fn compile_pattern(&self, pattern: &BindingPattern) -> Result<BytecodePattern> {
        match pattern {
            BindingPattern::Identifier(binding) => {
                Ok(BytecodePattern::Binding(self.compile_binding(binding)?))
            }
            BindingPattern::Object { properties, rest } => {
                let properties = properties
                    .iter()
                    .map(|property| self.compile_pattern_property(property))
                    .collect::<Result<Vec<_>>>()?;
                let rest = rest
                    .as_ref()
                    .map(|rest| {
                        self.compile_binding(rest)
                            .map(BytecodePattern::Binding)
                            .map(Rc::new)
                    })
                    .transpose()?;
                Ok(BytecodePattern::Object {
                    properties: properties.into(),
                    rest,
                })
            }
            BindingPattern::Array { elements, rest } => {
                let elements = elements
                    .iter()
                    .map(|element| {
                        element
                            .as_ref()
                            .map(|element| self.compile_pattern_element(element))
                            .transpose()
                    })
                    .collect::<Result<Vec<_>>>()?;
                let rest = rest
                    .as_ref()
                    .map(|rest| self.compile_pattern(rest).map(Rc::new))
                    .transpose()?;
                Ok(BytecodePattern::Array {
                    elements: elements.into(),
                    rest,
                })
            }
        }
    }

    pub(super) fn compile_assignment_pattern(
        &self,
        pattern: &AssignmentPattern,
        strict: bool,
    ) -> Result<BytecodePattern> {
        match pattern {
            AssignmentPattern::Target(target) => self
                .compile_assignment_target_with_strict(target, strict)
                .map(BytecodePattern::Assignment),
            AssignmentPattern::Object { properties, rest } => {
                let properties = properties
                    .iter()
                    .map(|property| self.compile_assignment_pattern_property(property, strict))
                    .collect::<Result<Vec<_>>>()?;
                let rest = rest
                    .as_ref()
                    .map(|target| {
                        self.compile_assignment_target_with_strict(target, strict)
                            .map(BytecodePattern::Assignment)
                            .map(Rc::new)
                    })
                    .transpose()?;
                Ok(BytecodePattern::Object {
                    properties: properties.into(),
                    rest,
                })
            }
            AssignmentPattern::Array { elements, rest } => {
                let elements = elements
                    .iter()
                    .map(|element| {
                        element
                            .as_ref()
                            .map(|element| self.compile_assignment_pattern_element(element, strict))
                            .transpose()
                    })
                    .collect::<Result<Vec<_>>>()?;
                let rest = rest
                    .as_ref()
                    .map(|rest| self.compile_assignment_pattern(rest, strict).map(Rc::new))
                    .transpose()?;
                Ok(BytecodePattern::Array {
                    elements: elements.into(),
                    rest,
                })
            }
        }
    }

    fn compile_pattern_property(
        &self,
        property: &ObjectBindingProperty,
    ) -> Result<BytecodePatternProperty> {
        let key = self.compile_pattern_key(&property.key)?;
        Ok(BytecodePatternProperty {
            key,
            target: self.compile_pattern_target(&property.target, property.default.as_ref())?,
        })
    }

    fn compile_assignment_pattern_property(
        &self,
        property: &ObjectAssignmentProperty,
        strict: bool,
    ) -> Result<BytecodePatternProperty> {
        Ok(BytecodePatternProperty {
            key: self.compile_pattern_key(&property.key)?,
            target: self.compile_assignment_pattern_target(
                &property.target,
                property.default.as_ref(),
                strict,
            )?,
        })
    }

    fn compile_pattern_element(
        &self,
        element: &ArrayBindingElement,
    ) -> Result<BytecodePatternTarget> {
        self.compile_pattern_target(&element.target, element.default.as_ref())
    }

    fn compile_assignment_pattern_element(
        &self,
        element: &ArrayAssignmentElement,
        strict: bool,
    ) -> Result<BytecodePatternTarget> {
        self.compile_assignment_pattern_target(&element.target, element.default.as_ref(), strict)
    }

    fn compile_pattern_target(
        &self,
        pattern: &BindingPattern,
        default: Option<&Expression>,
    ) -> Result<BytecodePatternTarget> {
        let inferred_name = match pattern {
            BindingPattern::Identifier(binding) => Some(binding.name()),
            BindingPattern::Object { .. } | BindingPattern::Array { .. } => None,
        };
        Ok(BytecodePatternTarget {
            pattern: self.compile_pattern(pattern)?,
            default: default
                .map(|default| {
                    inferred_name.map_or_else(
                        || BytecodeBlock::compile_expression(default, self.layout),
                        |name| {
                            BytecodeBlock::compile_expression_with_inferred_name(
                                default,
                                name,
                                self.layout,
                            )
                        },
                    )
                })
                .transpose()?,
        })
    }

    fn compile_assignment_pattern_target(
        &self,
        pattern: &AssignmentPattern,
        default: Option<&Expression>,
        strict: bool,
    ) -> Result<BytecodePatternTarget> {
        let inferred_name = match pattern {
            AssignmentPattern::Target(target) => match target.kind() {
                Expr::Identifier(binding) => Some(binding.name()),
                _ => None,
            },
            AssignmentPattern::Object { .. } | AssignmentPattern::Array { .. } => None,
        };
        Ok(BytecodePatternTarget {
            pattern: self.compile_assignment_pattern(pattern, strict)?,
            default: default
                .map(|default| {
                    inferred_name.map_or_else(
                        || BytecodeBlock::compile_expression(default, self.layout),
                        |name| {
                            BytecodeBlock::compile_expression_with_inferred_name(
                                default,
                                name,
                                self.layout,
                            )
                        },
                    )
                })
                .transpose()?,
        })
    }

    fn compile_pattern_key(&self, key: &PatternPropertyKey) -> Result<BytecodePatternKey> {
        match key {
            PatternPropertyKey::Static(name) => Ok(BytecodePatternKey::Static(name.clone())),
            PatternPropertyKey::Computed(expr) => {
                BytecodeBlock::compile_expression(expr, self.layout)
                    .map(BytecodePatternKey::Computed)
            }
        }
    }
}
