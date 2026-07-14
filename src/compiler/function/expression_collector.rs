use crate::ast::{Expr, Expression};

use super::CaptureBindingCollector;

impl CaptureBindingCollector {
    pub(super) fn collect_call_like_expr(&mut self, expr: &Expr) {
        let (callee, args) = match expr {
            Expr::Call { callee, args, .. } | Expr::OptionalCall { callee, args, .. } => {
                (callee.as_ref(), args.as_slice())
            }
            Expr::New { constructor, args } => (constructor.as_ref(), args.as_slice()),
            _ => return,
        };
        self.collect_expr(callee);
        self.collect_exprs(args);
    }

    pub(super) fn collect_conditional_expr(
        &mut self,
        condition: &Expression,
        consequent: &Expression,
        alternate: &Expression,
    ) {
        self.collect_expr(condition);
        self.collect_expr(consequent);
        self.collect_expr(alternate);
    }
}
