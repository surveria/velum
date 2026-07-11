use crate::{
    ast::{Expr, Expression},
    binding_metadata::BindingLayout,
    bytecode::{BytecodeBlock, BytecodeInstruction},
    error::Result,
    syntax::StaticName,
};

use super::BytecodeCompiler;

impl BytecodeBlock {
    pub(super) fn compile_expression_with_inferred_name(
        expr: &Expression,
        name: &StaticName,
        layout: &BindingLayout,
    ) -> Result<Self> {
        let mut compiler = BytecodeCompiler::new(layout, expr.span());
        compiler.compile_expr_with_inferred_name(expr, name)?;
        compiler.emit(BytecodeInstruction::StoreLast);
        compiler.finish()
    }
}

impl BytecodeCompiler<'_> {
    pub(super) fn compile_expr_with_inferred_name(
        &mut self,
        expr: &Expression,
        name: &StaticName,
    ) -> Result<()> {
        match expr.kind() {
            Expr::Function { name: None, .. } | Expr::ArrowFunction { .. } => self
                .with_source_span(expr.span(), |compiler| {
                    compiler.compile_function_literal_with_inferred_name(expr.kind(), name)
                }),
            Expr::Class(class) if class.name.is_none() => self
                .with_source_span(expr.span(), |compiler| {
                    compiler.compile_class_literal_with_inferred_name(class, Some(name))
                }),
            Expr::Parenthesized(inner) => self.compile_expr_with_inferred_name(inner, name),
            _ => self.compile_expr(expr),
        }
    }

    pub(super) fn is_anonymous_function_definition(expr: &Expression) -> bool {
        match expr.kind() {
            Expr::Function { name: None, .. } | Expr::ArrowFunction { .. } => true,
            Expr::Class(class) if class.name.is_none() => true,
            Expr::Parenthesized(inner) => Self::is_anonymous_function_definition(inner),
            _ => false,
        }
    }
}
