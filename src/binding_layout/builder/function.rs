use crate::{
    ast::{FunctionParam, Statement, StaticBinding, StaticFunctionId},
    binding_metadata::{FunctionScopeId, ScopeId, types::ScopeKind},
    error::Result,
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn analyze_function(
        &mut self,
        id: StaticFunctionId,
        self_binding: Option<&StaticBinding>,
        params: &[FunctionParam],
        body: &[Statement],
        parent_scope: ScopeId,
        parent_function: FunctionScopeId,
    ) -> Result<()> {
        let function = self.add_function(Some(parent_function));
        self.record_static_function(id, function)?;
        let function_parent_scope = if let Some(self_binding) = self_binding {
            let self_scope = self.add_scope(Some(parent_scope), function, ScopeKind::Local);
            self.declare(self_scope, self_binding)?;
            self_scope
        } else {
            parent_scope
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
}
