use crate::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Block(Vec<Self>),
    DeclList(Vec<Self>),
    If {
        condition: Expr,
        consequent: Box<Self>,
        alternate: Option<Box<Self>>,
    },
    While {
        condition: Expr,
        body: Box<Self>,
    },
    For {
        init: Option<Box<Self>>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Self>,
    },
    Switch {
        discriminant: Expr,
        cases: Vec<SwitchCase>,
    },
    Try {
        body: Vec<Self>,
        catch: Option<CatchClause>,
        finally_body: Option<Vec<Self>>,
    },
    Break,
    Continue,
    Throw(Expr),
    Return(Option<Expr>),
    VarDecl {
        name: String,
        kind: DeclKind,
        init: Option<Expr>,
    },
    Expr(Expr),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DeclKind {
    Var,
    Let,
    Const,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: String,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expr>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub param: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Identifier(String),
    Parenthesized(Box<Self>),
    Unary {
        op: UnaryOp,
        expr: Box<Self>,
    },
    Update {
        op: UpdateOp,
        prefix: bool,
        expr: Box<Self>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
    Conditional {
        condition: Box<Self>,
        consequent: Box<Self>,
        alternate: Box<Self>,
    },
    Assignment {
        name: String,
        expr: Box<Self>,
    },
    CompoundAssignment {
        op: BinaryOp,
        target: Box<Self>,
        expr: Box<Self>,
    },
    PropertyAssignment {
        object: Box<Self>,
        property: String,
        expr: Box<Self>,
    },
    ComputedPropertyAssignment {
        object: Box<Self>,
        property: Box<Self>,
        expr: Box<Self>,
    },
    Member {
        object: Box<Self>,
        property: String,
    },
    ComputedMember {
        object: Box<Self>,
        property: Box<Self>,
    },
    Call {
        callee: Box<Self>,
        args: Vec<Self>,
    },
    Function {
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Object(Vec<ObjectProperty>),
    Array(Vec<Self>),
    New {
        constructor: String,
        args: Vec<Self>,
    },
}

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
