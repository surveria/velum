use crate::{
    ast::{Expr, Expression},
    binding_metadata::{FunctionScopeId, ScopeId},
    error::Result,
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn analyze_expr(
        &mut self,
        expr: &Expression,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        match expr.kind() {
            Expr::Literal(_)
            | Expr::StringLiteral { .. }
            | Expr::TemplateObject { .. }
            | Expr::RegExpLiteral { .. }
            | Expr::This
            | Expr::ImportMeta
            | Expr::SuperMember { .. }
            | Expr::NewTarget
            | Expr::ArrayHole => Ok(()),
            Expr::SuperComputedMember { property, .. } => {
                self.analyze_expr(property, scope, function)
            }
            Expr::TemplateLiteral { expressions, .. } | Expr::Sequence(expressions) => {
                self.analyze_exprs(expressions, scope, function)
            }
            Expr::Identifier(binding) => self.resolve(binding, scope, function),
            Expr::Class(class) => self.analyze_class(class, scope, function),
            Expr::SuperCall { args } => self.analyze_exprs(args, scope, function),
            Expr::Parenthesized(expr)
            | Expr::Spread(expr)
            | Expr::Unary { expr, .. }
            | Expr::Await(expr)
            | Expr::Update { expr, .. }
            | Expr::SuperPropertyAssignment { expr, .. } => {
                self.analyze_expr(expr, scope, function)
            }
            Expr::Yield { expr, .. } => {
                self.analyze_optional_expr(expr.as_deref(), scope, function)
            }
            Expr::Binary { left, right, .. } => {
                self.analyze_expr(left, scope, function)?;
                self.analyze_expr(right, scope, function)
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.analyze_expr(condition, scope, function)?;
                self.analyze_expr(consequent, scope, function)?;
                self.analyze_expr(alternate, scope, function)
            }
            Expr::Assignment { name, expr, .. } => {
                self.resolve(name, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::DestructuringAssignment { pattern, expr, .. } => {
                pattern.for_each_expr(&mut |target| self.analyze_expr(target, scope, function))?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::CompoundAssignment { target, expr, .. } => {
                self.analyze_expr(target, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::WebCompatCallAssignment { target, discarded } => {
                self.analyze_web_compat(target, discarded.as_deref(), scope, function)
            }
            Expr::PropertyAssignment { object, expr, .. }
            | Expr::PrivateAssignment { object, expr, .. } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
                ..
            } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(property, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::SuperComputedPropertyAssignment { property, expr, .. } => {
                self.analyze_expr(property, scope, function)?;
                self.analyze_expr(expr, scope, function)
            }
            Expr::Member { object, .. }
            | Expr::PrivateMember { object, .. }
            | Expr::PrivateIn { object, .. } => self.analyze_expr(object, scope, function),
            Expr::ComputedMember {
                object, property, ..
            } => {
                self.analyze_expr(object, scope, function)?;
                self.analyze_expr(property, scope, function)
            }
            Expr::Call { callee, args, .. } => {
                self.analyze_expr(callee, scope, function)?;
                self.analyze_exprs(args, scope, function)
            }
            Expr::DynamicImport {
                specifier, options, ..
            } => {
                self.analyze_expr(specifier, scope, function)?;
                self.analyze_optional_expr(options.as_deref(), scope, function)
            }
            Expr::Function { .. } | Expr::ArrowFunction { .. } | Expr::MethodFunction { .. } => {
                self.analyze_nested_function(expr.kind(), scope, function)
            }
            Expr::Object(properties) => self.analyze_object_properties(properties, scope, function),
            Expr::Array(elements) => self.analyze_exprs(elements, scope, function),
            Expr::New { constructor, args } => {
                self.analyze_expr(constructor, scope, function)?;
                self.analyze_exprs(args, scope, function)
            }
        }
    }

    fn analyze_web_compat(
        &mut self,
        target: &Expression,
        discarded: Option<&Expression>,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        self.analyze_expr(target, scope, function)?;
        if let Some(discarded) = discarded {
            self.analyze_expr(discarded, scope, function)?;
        }
        Ok(())
    }

    fn analyze_optional_expr(
        &mut self,
        expr: Option<&Expression>,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        if let Some(expr) = expr {
            self.analyze_expr(expr, scope, function)?;
        }
        Ok(())
    }
}
