use crate::syntax::{BinaryOp, UnaryOp};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
}

impl BytecodeNumericBinaryOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Add => Some(Self::Add),
            BinaryOp::Sub => Some(Self::Sub),
            BinaryOp::Mul => Some(Self::Mul),
            BinaryOp::Div => Some(Self::Div),
            BinaryOp::Rem => Some(Self::Rem),
            BinaryOp::Pow => Some(Self::Pow),
            BinaryOp::BitAnd => Some(Self::BitAnd),
            BinaryOp::BitOr => Some(Self::BitOr),
            BinaryOp::BitXor => Some(Self::BitXor),
            BinaryOp::ShiftLeft => Some(Self::ShiftLeft),
            BinaryOp::ShiftRight => Some(Self::ShiftRight),
            BinaryOp::ShiftRightUnsigned => Some(Self::ShiftRightUnsigned),
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::InstanceOf
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::NullishCoalescing => None,
        }
    }

    pub(crate) const fn generic_binary(self) -> BinaryOp {
        match self {
            Self::Add => BinaryOp::Add,
            Self::Sub => BinaryOp::Sub,
            Self::Mul => BinaryOp::Mul,
            Self::Div => BinaryOp::Div,
            Self::Rem => BinaryOp::Rem,
            Self::Pow => BinaryOp::Pow,
            Self::BitAnd => BinaryOp::BitAnd,
            Self::BitOr => BinaryOp::BitOr,
            Self::BitXor => BinaryOp::BitXor,
            Self::ShiftLeft => BinaryOp::ShiftLeft,
            Self::ShiftRight => BinaryOp::ShiftRight,
            Self::ShiftRightUnsigned => BinaryOp::ShiftRightUnsigned,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericUnaryOp {
    Negate,
    Plus,
}

impl BytecodeNumericUnaryOp {
    pub(crate) const fn from_unary(op: UnaryOp) -> Option<Self> {
        match op {
            UnaryOp::Negate => Some(Self::Negate),
            UnaryOp::Plus => Some(Self::Plus),
            UnaryOp::Not | UnaryOp::Void | UnaryOp::Typeof | UnaryOp::Delete => None,
        }
    }

    pub(crate) const fn generic_unary(self) -> UnaryOp {
        match self {
            Self::Negate => UnaryOp::Negate,
            Self::Plus => UnaryOp::Plus,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericCompareOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl BytecodeNumericCompareOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Less => Some(Self::Less),
            BinaryOp::LessEqual => Some(Self::LessEqual),
            BinaryOp::Greater => Some(Self::Greater),
            BinaryOp::GreaterEqual => Some(Self::GreaterEqual),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Pow
            | BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::In
            | BinaryOp::InstanceOf
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::ShiftRightUnsigned
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::NullishCoalescing => None,
        }
    }

    pub(crate) const fn generic_binary(self) -> BinaryOp {
        match self {
            Self::Less => BinaryOp::Less,
            Self::LessEqual => BinaryOp::LessEqual,
            Self::Greater => BinaryOp::Greater,
            Self::GreaterEqual => BinaryOp::GreaterEqual,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BytecodeNumericEqualityOp {
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
}

impl BytecodeNumericEqualityOp {
    pub(crate) const fn from_binary(op: BinaryOp) -> Option<Self> {
        match op {
            BinaryOp::Equal => Some(Self::Equal),
            BinaryOp::NotEqual => Some(Self::NotEqual),
            BinaryOp::StrictEqual => Some(Self::StrictEqual),
            BinaryOp::StrictNotEqual => Some(Self::StrictNotEqual),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::Pow
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::InstanceOf
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight
            | BinaryOp::ShiftRightUnsigned
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::NullishCoalescing => None,
        }
    }

    pub(crate) const fn generic_binary(self) -> BinaryOp {
        match self {
            Self::Equal => BinaryOp::Equal,
            Self::NotEqual => BinaryOp::NotEqual,
            Self::StrictEqual => BinaryOp::StrictEqual,
            Self::StrictNotEqual => BinaryOp::StrictNotEqual,
        }
    }
}
