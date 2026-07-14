use std::rc::Rc;

use crate::ast::{Expr, Expression, FunctionParam, Statement};

use super::{CaptureBindingCollector, CollectedFunctionBindings};

impl CaptureBindingCollector {
    pub(super) fn collect_function_with_additional<'a>(
        params: &[FunctionParam],
        statements: &[Statement],
        additional_expressions: impl IntoIterator<Item = &'a Expression>,
    ) -> CollectedFunctionBindings {
        let mut collector = Self::default();
        collector.collect_param_defaults(params);
        collector.collect_statements(statements);
        for expression in additional_expressions {
            collector.collect_expr(expression);
        }
        CollectedFunctionBindings {
            bindings: Rc::from(collector.bindings.into_boxed_slice()),
            uses_arguments: collector.uses_arguments,
        }
    }

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
