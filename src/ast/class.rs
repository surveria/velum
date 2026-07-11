use std::rc::Rc;

use crate::syntax::{StaticFunctionId, StaticName};

use super::{Expression, FunctionParam, ObjectPropertyKey, Statement};

/// A parsed class body shared by class declarations and expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassLiteral {
    pub name: Option<StaticName>,
    pub heritage: Option<Expression>,
    pub constructor: ClassConstructor,
    pub members: Vec<ClassMember>,
    pub fields: Vec<ClassField>,
    pub static_blocks: Vec<ClassStaticBlock>,
}

/// A class static initialization block executed once with the constructor as
/// its `this` value when the class definition is evaluated.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassStaticBlock {
    pub body: Rc<[Statement]>,
}

/// A public class field: instance fields initialize against the new object
/// during construction, static fields initialize once against the
/// constructor at class creation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassField {
    pub key: ObjectPropertyKey,
    pub is_static: bool,
    pub name: Option<StaticName>,
    pub initializer: Option<Expression>,
}

/// The explicit `constructor` member, or a parser-synthesized empty default.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassConstructor {
    pub id: StaticFunctionId,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Statement]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub key: ObjectPropertyKey,
    pub kind: ClassMemberKind,
    pub is_static: bool,
    pub id: StaticFunctionId,
    pub name: Option<StaticName>,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Statement]>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClassMemberKind {
    Method,
    Getter,
    Setter,
}
