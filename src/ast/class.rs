#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::syntax::{FunctionKind, StaticBinding, StaticFunctionId, StaticName};

use super::{Expression, FunctionParam, ObjectPropertyKey, Statement};

/// A parsed class body shared by class declarations and expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassLiteral {
    pub decorators: Vec<Expression>,
    pub name: Option<StaticName>,
    pub inner_name_binding: Option<StaticBinding>,
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
    pub source_order: usize,
    pub body: Rc<[Statement]>,
}

/// A class element name: an ordinary property key or a lexically scoped
/// `#name` private identifier. Private names keep their leading `#`.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassElementName {
    Property(ObjectPropertyKey),
    Private(StaticName),
}

/// A class field: instance fields initialize against the new object during
/// construction, static fields initialize once against the constructor at
/// class creation. Private fields install lexically scoped slots instead of
/// ordinary properties.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassField {
    pub source_order: usize,
    pub key: ClassElementName,
    pub is_static: bool,
    pub auto_accessor: Option<ClassAutoAccessor>,
    pub name: Option<StaticName>,
    pub initializer: Option<Expression>,
    pub decorators: Vec<Expression>,
}

/// Hidden storage and synthesized access functions for one public
/// auto-accessor. The public key remains on `ClassField`, so computed keys are
/// evaluated once for the logical class element.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassAutoAccessor {
    pub backing_name: StaticName,
    pub getter: ClassAutoAccessorFunction,
    pub setter: ClassAutoAccessorFunction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassAutoAccessorFunction {
    pub id: StaticFunctionId,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Statement]>,
}

/// The explicit `constructor` member, or a parser-synthesized empty default.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassConstructor {
    pub id: StaticFunctionId,
    pub default_derived: bool,
    pub arguments_binding: Option<StaticBinding>,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Statement]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub source_order: usize,
    pub key: ClassElementName,
    pub kind: ClassMemberKind,
    pub function_kind: FunctionKind,
    pub is_static: bool,
    pub id: StaticFunctionId,
    pub arguments_binding: Option<StaticBinding>,
    pub name: Option<StaticName>,
    pub params: Rc<[FunctionParam]>,
    pub body: Rc<[Statement]>,
    pub decorators: Vec<Expression>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ClassMemberKind {
    Method,
    Getter,
    Setter,
}
