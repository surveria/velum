use std::rc::Rc;

use crate::syntax::{StaticFunctionId, StaticName};

use super::{FunctionParam, ObjectPropertyKey, Stmt};

/// A parsed class body shared by class declarations and expressions.
/// Inheritance, fields, and generator/async members are not represented yet;
/// the parser rejects them with explicit unsupported-feature errors.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassLiteral {
    pub name: Option<StaticName>,
    pub heritage: Option<super::Expr>,
    pub constructor: ClassConstructor,
    pub members: Vec<ClassMember>,
}

/// The explicit `constructor` member, or a parser-synthesized empty default.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassConstructor {
    pub id: StaticFunctionId,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Stmt]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub key: ObjectPropertyKey,
    pub kind: ClassMemberKind,
    pub is_static: bool,
    pub id: StaticFunctionId,
    pub name: Option<StaticName>,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Stmt]>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClassMemberKind {
    Method,
    Getter,
    Setter,
}
