mod expression;
mod function;
mod statement;

pub use expression::{Expr, ObjectProperty, ObjectPropertyKey, ObjectPropertyKind};
pub use function::FunctionParam;
pub use statement::{CatchClause, ForInTarget, Program, Stmt, SwitchCase};

pub use crate::syntax::{
    BinaryOp, DeclKind, StaticBinding, StaticBindingId, StaticCallSiteId, StaticFunctionId,
    StaticName, StaticNameId, StaticPropertyAccessId, StaticString, StaticStringId, UnaryOp,
    UpdateOp,
};
