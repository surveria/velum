mod ids;
mod static_values;

pub use ids::{
    StaticBindingId, StaticCallSiteId, StaticFunctionId, StaticNameId, StaticPropertyAccessId,
    StaticStringId,
};
pub use static_values::{StaticBinding, StaticName, StaticString};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UnaryOp {
    Negate,
    Plus,
    Not,
    Typeof,
    Void,
    Delete,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum UpdateOp {
    Increment,
    Decrement,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    In,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
    LogicalAnd,
    LogicalOr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeclKind {
    Var,
    Let,
    Const,
}
