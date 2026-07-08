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
    InstanceOf,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
    LogicalAnd,
    LogicalOr,
    NullishCoalescing,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeclKind {
    Var,
    Let,
    Const,
}

/// Which half of an accessor property a function value installs.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AccessorKind {
    Getter,
    Setter,
}
