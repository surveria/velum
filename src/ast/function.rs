use super::{Expr, StaticBinding};

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: StaticBinding,
    pub default: Option<Expr>,
    pub rest: bool,
}

impl FunctionParam {
    pub const fn new(name: StaticBinding, default: Option<Expr>) -> Self {
        Self {
            name,
            default,
            rest: false,
        }
    }

    pub const fn rest(name: StaticBinding) -> Self {
        Self {
            name,
            default: None,
            rest: true,
        }
    }
}
