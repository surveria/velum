use std::rc::Rc;

use crate::{
    syntax::{BinaryOp, UnaryOp, UpdateOp},
    value::Value,
};

use super::{
    FunctionParam, StaticBinding, StaticCallSiteId, StaticFunctionId, StaticName,
    StaticPropertyAccessId, StaticString, Stmt,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: ObjectPropertyKey,
    pub kind: ObjectPropertyKind,
    pub value: Expr,
}

/// How an object literal property definition installs its value: as a plain
/// data property or as the get/set half of an accessor property.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjectPropertyKind {
    Init,
    Get,
    Set,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPropertyKey {
    Static(StaticName),
    Computed(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    StringLiteral(StaticString),
    TemplateLiteral {
        quasis: Vec<StaticString>,
        expressions: Vec<Self>,
    },
    RegExpLiteral {
        pattern: StaticString,
        flags: StaticString,
    },
    This,
    NewTarget,
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
        site: StaticCallSiteId,
        args: Vec<Self>,
    },
    Function {
        id: StaticFunctionId,
        name: Option<StaticName>,
        params: Rc<[FunctionParam]>,
        body: Rc<[Stmt]>,
        is_async: bool,
    },
    ArrowFunction {
        id: StaticFunctionId,
        params: Rc<[FunctionParam]>,
        body: Rc<[Stmt]>,
        is_async: bool,
    },
    MethodFunction {
        id: StaticFunctionId,
        name: Option<StaticName>,
        params: Rc<[FunctionParam]>,
        body: Rc<[Stmt]>,
        is_async: bool,
    },
    Object(Vec<ObjectProperty>),
    Array(Vec<Self>),
    New {
        constructor: Box<Self>,
        args: Vec<Self>,
    },
}
