use std::rc::Rc;

use crate::{
    ast::{
        ArrayBindingElement, BindingPattern, BindingPropertyKey, Expression, ObjectBindingProperty,
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
                    .map(|rest| self.compile_binding(rest))
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

    fn compile_pattern_property(
        &self,
        property: &ObjectBindingProperty,
    ) -> Result<BytecodePatternProperty> {
        let key = match &property.key {
            BindingPropertyKey::Static(name) => BytecodePatternKey::Static(name.clone()),
            BindingPropertyKey::Computed(expr) => {
                BytecodePatternKey::Computed(BytecodeBlock::compile_expression(expr, self.layout)?)
            }
        };
        Ok(BytecodePatternProperty {
            key,
            target: self.compile_pattern_target(&property.target, property.default.as_ref())?,
        })
    }

    fn compile_pattern_element(
        &self,
        element: &ArrayBindingElement,
    ) -> Result<BytecodePatternTarget> {
        self.compile_pattern_target(&element.target, element.default.as_ref())
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
}
