use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Program {
    pub(crate) statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Stmt {
    VarDecl {
        name: String,
        mutable: bool,
        init: Expr,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expr {
    Literal(Value),
    Identifier(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Assignment {
        name: String,
        expr: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum UnaryOp {
    Negate,
    Plus,
    Not,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Equal,
    NotEqual,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    LogicalAnd,
    LogicalOr,
}
