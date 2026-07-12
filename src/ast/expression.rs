use std::rc::Rc;

use crate::{
    syntax::{BinaryOp, FunctionKind, UnaryOp, UpdateOp},
    value::Value,
};

use super::{
    AssignmentPattern, AstNode, FunctionParam, Statement, StaticBinding, StaticCallSiteId,
    StaticFunctionId, StaticName, StaticPropertyAccessId, StaticString,
};

pub type Expression = AstNode<Expr>;

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProperty {
    pub key: ObjectPropertyKey,
    pub kind: ObjectPropertyKind,
    pub value: Expression,
}

/// How an object literal property definition installs its value: as a plain
/// data property or as the get/set half of an accessor property.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ObjectPropertyKind {
    Init,
    Get,
    Set,
    /// A `...expr` entry copying own enumerable properties of the value.
    Spread,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPropertyKey {
    Static(StaticName),
    Computed(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    StringLiteral(StaticString),
    Spread(Box<Expression>),
    Class(Box<crate::ast::ClassLiteral>),
    SuperCall {
        args: Vec<Expression>,
    },
    SuperMember {
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    SuperComputedMember {
        property: Box<Expression>,
        access: StaticPropertyAccessId,
    },
    TemplateLiteral {
        quasis: Vec<StaticString>,
        expressions: Vec<Expression>,
    },
    RegExpLiteral {
        pattern: StaticString,
        flags: StaticString,
    },
    This,
    NewTarget,
    Identifier(StaticBinding),
    Parenthesized(Box<Expression>),
    Sequence(Vec<Expression>),
    Unary {
        op: UnaryOp,
        expr: Box<Expression>,
    },
    Await(Box<Expression>),
    Yield {
        expr: Option<Box<Expression>>,
        delegate: bool,
    },
    Update {
        op: UpdateOp,
        prefix: bool,
        strict: bool,
        expr: Box<Expression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expression>,
        right: Box<Expression>,
        property_access: Option<StaticPropertyAccessId>,
    },
    Conditional {
        condition: Box<Expression>,
        consequent: Box<Expression>,
        alternate: Box<Expression>,
    },
    Assignment {
        name: StaticBinding,
        strict: bool,
        infer_name: bool,
        expr: Box<Expression>,
    },
    DestructuringAssignment {
        pattern: Box<AssignmentPattern>,
        strict: bool,
        expr: Box<Expression>,
    },
    CompoundAssignment {
        op: BinaryOp,
        strict: bool,
        target: Box<Expression>,
        expr: Box<Expression>,
    },
    PropertyAssignment {
        object: Box<Expression>,
        property: StaticName,
        access: StaticPropertyAccessId,
        strict: bool,
        expr: Box<Expression>,
    },
    ComputedPropertyAssignment {
        object: Box<Expression>,
        property: Box<Expression>,
        access: StaticPropertyAccessId,
        strict: bool,
        expr: Box<Expression>,
    },
    SuperPropertyAssignment {
        property: StaticName,
        access: StaticPropertyAccessId,
        strict: bool,
        expr: Box<Expression>,
    },
    SuperComputedPropertyAssignment {
        property: Box<Expression>,
        access: StaticPropertyAccessId,
        strict: bool,
        expr: Box<Expression>,
    },
    Member {
        object: Box<Expression>,
        property: StaticName,
        access: StaticPropertyAccessId,
    },
    ComputedMember {
        object: Box<Expression>,
        property: Box<Expression>,
        access: StaticPropertyAccessId,
    },
    /// A private member read such as `obj.#name`; the name keeps its `#`.
    PrivateMember {
        object: Box<Expression>,
        name: StaticName,
    },
    /// A private member write such as `obj.#name = value`.
    PrivateAssignment {
        object: Box<Expression>,
        name: StaticName,
        expr: Box<Expression>,
    },
    /// An ergonomic brand check such as `#name in object`.
    PrivateIn {
        name: StaticName,
        object: Box<Expression>,
    },
    Call {
        callee: Box<Expression>,
        site: StaticCallSiteId,
        strict: bool,
        args: Vec<Expression>,
    },
    Function {
        id: StaticFunctionId,
        name: Option<StaticBinding>,
        arguments_binding: Option<StaticBinding>,
        params: Rc<[FunctionParam]>,
        body: Rc<[Statement]>,
        parameter_prologue_count: usize,
        kind: FunctionKind,
        strict: bool,
    },
    ArrowFunction {
        id: StaticFunctionId,
        params: Rc<[FunctionParam]>,
        body: Rc<[Statement]>,
        parameter_prologue_count: usize,
        kind: FunctionKind,
        strict: bool,
    },
    MethodFunction {
        id: StaticFunctionId,
        name: Option<StaticName>,
        arguments_binding: Option<StaticBinding>,
        params: Rc<[FunctionParam]>,
        body: Rc<[Statement]>,
        parameter_prologue_count: usize,
        kind: FunctionKind,
        strict: bool,
    },
    Object(Vec<ObjectProperty>),
    ArrayHole,
    Array(Vec<Expression>),
    New {
        constructor: Box<Expression>,
        args: Vec<Expression>,
    },
}
