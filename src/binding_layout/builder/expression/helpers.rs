use crate::{
    ast::{Expr, Expression},
    binding_metadata::{FunctionScopeId, ScopeId},
    error::{Error, Result},
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn analyze_call_like(
        &mut self,
        expr: &Expr,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let (callee, args) = match expr {
            Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } => {
                (callee.as_ref(), args.as_slice())
            }
            Expr::New { constructor, args } => (constructor.as_ref(), args.as_slice()),
            _ => return Err(Error::runtime("expression is not call-like")),
        };
        self.analyze_expr(callee, scope, function)?;
        self.analyze_exprs(args, scope, function)
    }

    pub(super) fn analyze_conditional(
        &mut self,
        condition: &Expression,
        consequent: &Expression,
        alternate: &Expression,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(condition, scope, function)?;
        self.analyze_expr(consequent, scope, function)?;
        self.analyze_expr(alternate, scope, function)
    }
}
