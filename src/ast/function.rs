use super::{Expr, StaticBinding};

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: StaticBinding,
    pub default: Option<Expr>,
}

impl FunctionParam {
    pub const fn new(name: StaticBinding, default: Option<Expr>) -> Self {
        Self { name, default }
    }
}
