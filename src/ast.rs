use std::rc::Rc;

use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticNameId(u32);

impl StaticNameId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static name table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("static name id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticName {
    id: StaticNameId,
    text: Rc<str>,
}

impl StaticName {
    pub fn new(id: StaticNameId, name: String) -> Self {
        Self {
            id,
            text: Rc::from(name.into_boxed_str()),
        }
    }

    pub fn borrowed(id: StaticNameId, name: &str) -> Self {
        Self {
            id,
            text: Rc::from(name),
        }
    }

    pub const fn id(&self) -> StaticNameId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }
}

impl std::fmt::Display for StaticName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

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
    ForIn {
        target: ForInTarget,
        object: Expr,
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
        name: StaticName,
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
pub enum ForInTarget {
    Binding { name: StaticName, kind: DeclKind },
    Assignment(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: StaticName,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expr>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub param: Option<StaticName>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    This,
    Identifier(StaticName),
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
        name: StaticName,
        expr: Box<Self>,
    },
    CompoundAssignment {
        op: BinaryOp,
        target: Box<Self>,
        expr: Box<Self>,
    },
    PropertyAssignment {
        object: Box<Self>,
        property: StaticName,
        expr: Box<Self>,
    },
    ComputedPropertyAssignment {
        object: Box<Self>,
        property: Box<Self>,
        expr: Box<Self>,
    },
    Member {
        object: Box<Self>,
        property: StaticName,
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
        name: Option<StaticName>,
        params: Rc<[StaticName]>,
        body: Rc<[Stmt]>,
    },
    MethodFunction {
        name: StaticName,
        params: Rc<[StaticName]>,
        body: Rc<[Stmt]>,
    },
    Object(Vec<ObjectProperty>),
    Array(Vec<Self>),
    New {
        constructor: StaticName,
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
