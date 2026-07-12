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
    BitNot,
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
    Using,
    AwaitUsing,
}

impl DeclKind {
    pub const fn is_mutable(self) -> bool {
        matches!(self, Self::Var | Self::Let)
    }

    pub const fn is_resource(self) -> bool {
        matches!(self, Self::Using | Self::AwaitUsing)
    }

    pub const fn is_async_resource(self) -> bool {
        matches!(self, Self::AwaitUsing)
    }

    pub const fn requires_initializer(self) -> bool {
        matches!(self, Self::Const | Self::Using | Self::AwaitUsing)
    }
}

/// Execution semantics attached to a JavaScript function definition.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FunctionKind {
    Ordinary,
    Async,
    Generator,
    AsyncGenerator,
}

impl FunctionKind {
    pub const fn is_async_generator(self) -> bool {
        matches!(self, Self::AsyncGenerator)
    }

    pub const fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncGenerator)
    }

    pub const fn is_generator(self) -> bool {
        matches!(self, Self::Generator | Self::AsyncGenerator)
    }

    pub const fn is_constructable(self) -> bool {
        matches!(self, Self::Ordinary)
    }
}

/// Which half of an accessor property a function value installs.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AccessorKind {
    Getter,
    Setter,
}
