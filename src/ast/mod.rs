mod class;
mod expression;
mod function;
mod node;
mod pattern;
mod statement;

pub use class::{ClassConstructor, ClassField, ClassLiteral, ClassMember, ClassMemberKind};
pub use expression::{Expr, Expression, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind};
pub use function::FunctionParam;
pub use node::AstNode;
pub use pattern::{
    ArrayAssignmentElement, ArrayBindingElement, AssignmentPattern, BindingPattern,
    ObjectAssignmentProperty, ObjectBindingProperty, PatternPropertyKey,
};
pub use statement::{CatchClause, ForInTarget, Program, Statement, Stmt, SwitchCase};

pub use crate::syntax::{
    BinaryOp, DeclKind, FunctionKind, StaticBinding, StaticBindingId, StaticCallSiteId,
    StaticFunctionId, StaticName, StaticNameId, StaticPropertyAccessId, StaticString,
    StaticStringId, UnaryOp, UpdateOp,
};
