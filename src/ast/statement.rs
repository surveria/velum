use std::rc::Rc;

use crate::syntax::DeclKind;

use super::{
    AssignmentPattern, AstNode, BindingPattern, ClassLiteral, Expression, FunctionKind,
    FunctionParam, StaticBinding, StaticFunctionId, StaticName,
};

pub type Statement = AstNode<Stmt>;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Empty,
    Block(Vec<Statement>),
    DeclList(Vec<Statement>),
    If {
        condition: Expression,
        consequent: Box<Statement>,
        alternate: Option<Box<Statement>>,
    },
    While {
        condition: Expression,
        body: Box<Statement>,
    },
    DoWhile {
        body: Box<Statement>,
        condition: Expression,
    },
    With {
        object: Expression,
        body: Box<Statement>,
    },
    Label {
        label: StaticName,
        body: Box<Statement>,
    },
    For {
        init: Option<Box<Statement>>,
        condition: Option<Expression>,
        update: Option<Expression>,
        body: Box<Statement>,
    },
    ForIn {
        target: ForInTarget,
        object: Expression,
        body: Box<Statement>,
    },
    ForOf {
        target: ForInTarget,
        object: Expression,
        body: Box<Statement>,
        asynchronous: bool,
    },
    Switch {
        discriminant: Expression,
        cases: Vec<SwitchCase>,
    },
    Try {
        body: Vec<Statement>,
        catch: Option<CatchClause>,
        finally_body: Option<Vec<Statement>>,
    },
    Break(Option<StaticName>),
    Continue(Option<StaticName>),
    Throw(Expression),
    Return(Option<Expression>),
    FunctionDecl {
        name: StaticBinding,
        arguments_binding: Option<StaticBinding>,
        id: StaticFunctionId,
        params: Rc<[FunctionParam]>,
        body: Rc<[Statement]>,
        parameter_prologue_count: usize,
        kind: FunctionKind,
        strict: bool,
    },
    VarDecl {
        name: StaticBinding,
        kind: DeclKind,
        init: Option<Expression>,
    },
    PatternDecl {
        pattern: BindingPattern,
        kind: DeclKind,
        init: Expression,
    },
    ClassDecl {
        name: StaticBinding,
        class: Box<ClassLiteral>,
    },
    Expr(Expression),
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
    PatternAssignment {
        pattern: Box<AssignmentPattern>,
        strict: bool,
    },
    Assignment(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub test: Option<Expression>,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub param: Option<StaticBinding>,
    pub body: Vec<Statement>,
}
