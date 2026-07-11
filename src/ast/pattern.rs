use super::{Expression, StaticBinding, StaticName};

/// A destructuring binding target used by declarations, function parameters,
/// and `for-in`/`for-of` heads.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    Identifier(StaticBinding),
    Object {
        properties: Vec<ObjectBindingProperty>,
        rest: Option<StaticBinding>,
    },
    Array {
        /// `None` entries are elisions that consume one iterator step.
        elements: Vec<Option<ArrayBindingElement>>,
        rest: Option<Box<Self>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectBindingProperty {
    pub key: PatternPropertyKey,
    pub target: BindingPattern,
    pub default: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternPropertyKey {
    Static(StaticName),
    Computed(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayBindingElement {
    pub target: BindingPattern,
    pub default: Option<Expression>,
}

impl BindingPattern {
    /// Visits every identifier bound by this pattern, including object rest
    /// bindings and nested targets, in source order.
    pub fn for_each_binding<E>(
        &self,
        visit: &mut impl FnMut(&StaticBinding) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Identifier(binding) => visit(binding),
            Self::Object { properties, rest } => {
                for property in properties {
                    property.target.for_each_binding(visit)?;
                }
                if let Some(rest) = rest {
                    visit(rest)?;
                }
                Ok(())
            }
            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.target.for_each_binding(visit)?;
                }
                if let Some(rest) = rest {
                    rest.for_each_binding(visit)?;
                }
                Ok(())
            }
        }
    }

    /// Visits every expression embedded in the pattern: computed property
    /// keys and default initializers, in source order.
    pub fn for_each_expr<E>(
        &self,
        visit: &mut impl FnMut(&Expression) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Identifier(_) => Ok(()),
            Self::Object { properties, .. } => {
                for property in properties {
                    if let PatternPropertyKey::Computed(key) = &property.key {
                        visit(key)?;
                    }
                    if let Some(default) = &property.default {
                        visit(default)?;
                    }
                    property.target.for_each_expr(visit)?;
                }
                Ok(())
            }
            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    if let Some(default) = &element.default {
                        visit(default)?;
                    }
                    element.target.for_each_expr(visit)?;
                }
                if let Some(rest) = rest {
                    rest.for_each_expr(visit)?;
                }
                Ok(())
            }
        }
    }
}

/// A destructuring assignment target. Unlike a binding pattern, leaves may be
/// existing bindings or property references and therefore retain expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum AssignmentPattern {
    Target(Expression),
    Object {
        properties: Vec<ObjectAssignmentProperty>,
        rest: Option<Box<Expression>>,
    },
    Array {
        /// `None` entries are elisions that consume one iterator step.
        elements: Vec<Option<ArrayAssignmentElement>>,
        rest: Option<Box<Self>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectAssignmentProperty {
    pub key: PatternPropertyKey,
    pub target: AssignmentPattern,
    pub default: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayAssignmentElement {
    pub target: AssignmentPattern,
    pub default: Option<Expression>,
}

impl AssignmentPattern {
    /// Visits assignment targets, computed keys, and defaults in source order.
    pub fn for_each_expr<E>(
        &self,
        visit: &mut impl FnMut(&Expression) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Target(target) => visit(target),
            Self::Object { properties, rest } => {
                for property in properties {
                    if let PatternPropertyKey::Computed(key) = &property.key {
                        visit(key)?;
                    }
                    property.target.for_each_expr(visit)?;
                    if let Some(default) = &property.default {
                        visit(default)?;
                    }
                }
                if let Some(rest) = rest {
                    visit(rest)?;
                }
                Ok(())
            }
            Self::Array { elements, rest } => {
                for element in elements.iter().flatten() {
                    element.target.for_each_expr(visit)?;
                    if let Some(default) = &element.default {
                        visit(default)?;
                    }
                }
                if let Some(rest) = rest {
                    rest.for_each_expr(visit)?;
                }
                Ok(())
            }
        }
    }
}
