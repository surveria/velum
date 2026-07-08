use std::rc::Rc;

use crate::syntax::DeclKind;

use super::{BindingPattern, Expr, FunctionParam, StaticBinding, StaticFunctionId, StaticName};

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Empty,
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
    DoWhile {
        body: Box<Self>,
        condition: Expr,
    },
    Label {
        label: StaticName,
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
    ForOf {
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
    Break(Option<StaticName>),
    Continue(Option<StaticName>),
    Throw(Expr),
    Return(Option<Expr>),
    FunctionDecl {
        name: StaticBinding,
        id: StaticFunctionId,
        params: Rc<[FunctionParam]>,
        body: Rc<[Self]>,
        is_async: bool,
    },
    VarDecl {
        name: StaticBinding,
        kind: DeclKind,
        init: Option<Expr>,
    },
    PatternDecl {
        pattern: BindingPattern,
        kind: DeclKind,
        init: Expr,
    },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForInTarget {
    Binding {
        name: StaticBinding,
        kind: DeclKind,
    },
    PatternBinding {
        pattern: Box<BindingPattern>,
        kind: DeclKind,
    },
    Assignment(Expr),
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
