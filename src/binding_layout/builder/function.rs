use crate::{
    ast::{
        Expr, Expression, FunctionParam, ObjectProperty, ObjectPropertyKey, Statement,
        StaticBinding, StaticFunctionId,
    },
    binding_metadata::{FunctionScopeId, ScopeId, types::ScopeKind},
    error::Result,
};

use super::LayoutBuilder;

#[derive(Clone, Copy)]
pub(super) struct FunctionBindings<'a> {
    pub(super) self_binding: Option<&'a StaticBinding>,
    pub(super) arguments_binding: Option<&'a StaticBinding>,
}

impl<'a> FunctionBindings<'a> {
    pub(super) const fn new(
        self_binding: Option<&'a StaticBinding>,
        arguments_binding: Option<&'a StaticBinding>,
    ) -> Self {
        Self {
            self_binding,
            arguments_binding,
        }
    }
}

impl LayoutBuilder {
    pub(super) fn analyze_function_declaration(
        &mut self,
        statement: &crate::ast::Stmt,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        let crate::ast::Stmt::FunctionDecl {
            name,
            id,
            arguments_binding,
            params,
            body,
            ..
        } = statement
        else {
            return Err(crate::Error::runtime(
                "expected function declaration during binding analysis",
            ));
        };
        self.resolve_declaration_if_with_sensitive(name, scope, function)?;
        self.analyze_function(
            *id,
            FunctionBindings::new(None, arguments_binding.as_ref()),
            params,
            body,
            scope,
            function,
        )
    }

    pub(in crate::binding_layout) fn function_mut(
        &mut self,
        id: FunctionScopeId,
    ) -> Result<&mut crate::binding_metadata::types::FunctionScope> {
        self.functions
            .get_mut(id.index())
            .ok_or_else(|| crate::error::Error::runtime("binding layout function is not defined"))
    }

    pub(super) fn analyze_exprs(
        &mut self,
        exprs: &[Expression],
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        exprs
            .iter()
            .try_for_each(|expr| self.analyze_expr(expr, scope, function))
    }

    pub(super) fn analyze_function(
        &mut self,
        id: StaticFunctionId,
        bindings: FunctionBindings<'_>,
        params: &[FunctionParam],
        body: &[Statement],
        parent_scope: ScopeId,
        parent_function: FunctionScopeId,
    ) -> Result<()> {
        let function = self.add_function(Some(parent_function));
        self.record_static_function(id, function)?;
        let function_parent_scope = if let Some(self_binding) = bindings.self_binding {
            let self_scope = self.add_scope(Some(parent_scope), function, ScopeKind::Local);
            self.declare(self_scope, self_binding)?;
            self_scope
        } else {
            parent_scope
        };
        let function_parent_scope = if let Some(arguments_binding) = bindings.arguments_binding {
            let arguments_scope =
                self.add_scope(Some(function_parent_scope), function, ScopeKind::Local);
            self.declare(arguments_scope, arguments_binding)?;
            arguments_scope
        } else {
            function_parent_scope
        };
        let function_scope =
            self.add_scope(Some(function_parent_scope), function, ScopeKind::Local);
        for param in params {
            self.declare(function_scope, &param.name)?;
        }
        for param in params {
            if let Some(default) = &param.default {
                self.analyze_expr(default, function_scope, function)?;
            }
        }
        self.analyze_statements(body, function_scope, function_scope, function)
    }

    pub(super) fn analyze_nested_function(
        &mut self,
        expression: &Expr,
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        match expression {
            Expr::Function {
                id,
                name,
                arguments_binding,
                params,
                body,
                ..
            } => self.analyze_function(
                *id,
                FunctionBindings::new(name.as_ref(), arguments_binding.as_ref()),
                params,
                body,
                scope,
                function,
            ),
            Expr::ArrowFunction {
                id, params, body, ..
            } => self.analyze_function(
                *id,
                FunctionBindings::new(None, None),
                params,
                body,
                scope,
                function,
            ),
            Expr::MethodFunction {
                id,
                arguments_binding,
                params,
                body,
                ..
            } => self.analyze_function(
                *id,
                FunctionBindings::new(None, arguments_binding.as_ref()),
                params,
                body,
                scope,
                function,
            ),
            _ => Err(crate::error::Error::runtime(
                "expected nested function expression",
            )),
        }
    }

    pub(super) fn analyze_object_properties(
        &mut self,
        properties: &[ObjectProperty],
        scope: ScopeId,
        function: FunctionScopeId,
    ) -> Result<()> {
        for property in properties {
            if let ObjectPropertyKey::Computed(expr) = &property.key {
                self.analyze_expr(expr, scope, function)?;
            }
            self.analyze_expr(&property.value, scope, function)?;
        }
        Ok(())
    }
}
