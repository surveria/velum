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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticStringId(u32);

impl StaticStringId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static string table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static string id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticBindingId(u32);

impl StaticBindingId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static binding table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static binding id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticFunctionId(u32);

impl StaticFunctionId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static function table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static function id exceeded addressable range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct StaticPropertyAccessId(u32);

impl StaticPropertyAccessId {
    pub fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("static property access table exceeded supported range"))?;
        Ok(Self(id))
    }

    pub fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("static property access id exceeded supported range"))
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticString {
    id: StaticStringId,
    text: Rc<str>,
}

impl StaticString {
    pub fn new(id: StaticStringId, value: String) -> Self {
        Self {
            id,
            text: Rc::from(value.into_boxed_str()),
        }
    }

    pub const fn id(&self) -> StaticStringId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }
}

impl std::fmt::Display for StaticString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticBinding {
    id: StaticBindingId,
    name: StaticName,
}

impl StaticBinding {
    pub const fn new(id: StaticBindingId, name: StaticName) -> Self {
        Self { id, name }
    }

    pub const fn id(&self) -> StaticBindingId {
        self.id
    }

    pub const fn name(&self) -> &StaticName {
        &self.name
    }

    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl std::fmt::Display for StaticBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticBinding {
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
    FunctionDecl {
        name: StaticBinding,
        id: StaticFunctionId,
        params: Rc<[StaticBinding]>,
        body: Rc<[Self]>,
        is_async: bool,
    },
    VarDecl {
        name: StaticBinding,
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
    Binding { name: StaticBinding, kind: DeclKind },
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
    pub param: Option<StaticBinding>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    StringLiteral(StaticString),
    This,
    Identifier(StaticBinding),
    Parenthesized(Box<Self>),
    Unary {
        op: UnaryOp,
        expr: Box<Self>,
    },
    Await(Box<Self>),
    Update {
        op: UpdateOp,
        prefix: bool,
        expr: Box<Self>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Self>,
        right: Box<Self>,
        property_access: Option<StaticPropertyAccessId>,
    },
    Conditional {
        condition: Box<Self>,
        consequent: Box<Self>,
        alternate: Box<Self>,
    },
    Assignment {
        name: StaticBinding,
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
        access: StaticPropertyAccessId,
        expr: Box<Self>,
    },
    ComputedPropertyAssignment {
        object: Box<Self>,
        property: Box<Self>,
        access: StaticPropertyAccessId,
        expr: Box<Self>,
    },
    Member {
        object: Box<Self>,
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedMember {
        object: Box<Self>,
        property: Box<Self>,
        access: StaticPropertyAccessId,
    },
    Call {
        callee: Box<Self>,
        args: Vec<Self>,
    },
    Function {
        id: StaticFunctionId,
        name: Option<StaticName>,
        params: Rc<[StaticBinding]>,
        body: Rc<[Stmt]>,
        is_async: bool,
    },
    ArrowFunction {
        id: StaticFunctionId,
        params: Rc<[StaticBinding]>,
        body: Rc<[Stmt]>,
        is_async: bool,
    },
    MethodFunction {
        id: StaticFunctionId,
        name: StaticName,
        params: Rc<[StaticBinding]>,
        body: Rc<[Stmt]>,
    },
    Object(Vec<ObjectProperty>),
    Array(Vec<Self>),
    New {
        constructor: StaticBinding,
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
