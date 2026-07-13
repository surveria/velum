use super::{BindingPattern, Expression, StaticBinding};

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionParamTarget {
    Binding(StaticBinding),
    Pattern(BindingPattern),
}

impl FunctionParamTarget {
    pub fn for_each_binding<E>(
        &self,
        visit: &mut impl FnMut(&StaticBinding) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Binding(binding) => visit(binding),
            Self::Pattern(pattern) => pattern.for_each_binding(visit),
        }
    }

    pub const fn binding(&self) -> Option<&StaticBinding> {
        match self {
            Self::Binding(binding) => Some(binding),
            Self::Pattern(_) => None,
        }
    }

    pub const fn pattern(&self) -> Option<&BindingPattern> {
        match self {
            Self::Binding(_) => None,
            Self::Pattern(pattern) => Some(pattern),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub target: FunctionParamTarget,
    pub default: Option<Expression>,
    pub rest: bool,
}

impl FunctionParam {
    pub const fn new(name: StaticBinding, default: Option<Expression>) -> Self {
        Self {
            target: FunctionParamTarget::Binding(name),
            default,
            rest: false,
        }
    }

    pub const fn pattern(pattern: BindingPattern, default: Option<Expression>) -> Self {
        Self {
            target: FunctionParamTarget::Pattern(pattern),
            default,
            rest: false,
        }
    }

    pub const fn rest(name: StaticBinding) -> Self {
        Self {
            target: FunctionParamTarget::Binding(name),
            default: None,
            rest: true,
        }
    }

    pub const fn rest_pattern(pattern: BindingPattern) -> Self {
        Self {
            target: FunctionParamTarget::Pattern(pattern),
            default: None,
            rest: true,
        }
    }

    pub const fn is_simple_binding(&self) -> bool {
        matches!(self.target, FunctionParamTarget::Binding(_))
            && self.default.is_none()
            && !self.rest
    }

    pub const fn requires_runtime_initialization(&self) -> bool {
        self.default.is_some() || matches!(self.target, FunctionParamTarget::Pattern(_))
    }
}
