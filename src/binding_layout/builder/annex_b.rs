#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::collections::BTreeSet;

use crate::{
    ast::{BindingPattern, DeclKind, ForInTarget, Statement, Stmt},
    binding_metadata::ScopeId,
    error::Result,
};

use super::LayoutBuilder;

impl LayoutBuilder {
    pub(super) fn collect_direct_declaration(
        &mut self,
        statement: &Statement,
        scope: ScopeId,
        var_scope: ScopeId,
    ) -> Result<()> {
        if let Some(Stmt::FunctionDecl {
            name, block_scoped, ..
        }) = statement.kind().function_declaration_through_labels()
        {
            return self.declare(if *block_scoped { scope } else { var_scope }, name);
        }
        match statement.kind() {
            Stmt::DeclList(statements) => {
                for declaration in statements {
                    self.collect_direct_declaration(declaration, scope, var_scope)?;
                }
                Ok(())
            }
            Stmt::VarDecl { name, kind, .. } => match kind {
                DeclKind::Var => self.declare(var_scope, name),
                DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing => {
                    self.declare(scope, name)
                }
            },
            Stmt::PatternDecl { pattern, kind, .. } => match kind {
                DeclKind::Var => self.declare_pattern(pattern, var_scope),
                DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing => {
                    self.declare_pattern(pattern, scope)
                }
            },
            Stmt::ImportBinding { name } | Stmt::ClassDecl { name, .. } => {
                self.declare(scope, name)
            }
            Stmt::FunctionDecl { .. }
            | Stmt::Empty
            | Stmt::Debugger
            | Stmt::Block(_)
            | Stmt::If { .. }
            | Stmt::While { .. }
            | Stmt::DoWhile { .. }
            | Stmt::With { .. }
            | Stmt::Label { .. }
            | Stmt::For { .. }
            | Stmt::ForIn { .. }
            | Stmt::ForOf { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::Expr(_) => Ok(()),
        }
    }

    pub(super) fn collect_annex_b_var_bindings(
        &mut self,
        statements: &[Statement],
        var_scope: ScopeId,
    ) -> Result<()> {
        self.collect_annex_b_statement_list(statements, var_scope, &BTreeSet::new())
    }

    fn collect_annex_b_statement_list(
        &mut self,
        statements: &[Statement],
        var_scope: ScopeId,
        inherited: &BTreeSet<String>,
    ) -> Result<()> {
        let mut blocked = inherited.clone();
        for statement in statements {
            Self::collect_direct_lexical_names(statement, &mut blocked)?;
        }
        let mut nested_blocked = blocked.clone();
        for statement in statements {
            if let Some(Stmt::FunctionDecl {
                name,
                block_scoped: true,
                ..
            }) = statement.kind().function_declaration_through_labels()
            {
                nested_blocked.insert(name.name().as_str().to_owned());
            }
        }
        for statement in statements {
            if let Some(function @ Stmt::FunctionDecl { .. }) =
                statement.kind().function_declaration_through_labels()
            {
                self.collect_annex_b_function(function, var_scope, &blocked)?;
            } else {
                self.collect_annex_b_statement(statement, var_scope, &nested_blocked)?;
            }
        }
        Ok(())
    }

    fn collect_annex_b_function(
        &mut self,
        function: &Stmt,
        var_scope: ScopeId,
        blocked: &BTreeSet<String>,
    ) -> Result<()> {
        let Stmt::FunctionDecl {
            name,
            annex_b_var_binding: Some(variable),
            ..
        } = function
        else {
            return Ok(());
        };
        if !blocked.contains(name.name().as_str()) {
            self.declare(var_scope, variable)?;
        }
        Ok(())
    }

    fn collect_annex_b_statement(
        &mut self,
        statement: &Statement,
        var_scope: ScopeId,
        blocked: &BTreeSet<String>,
    ) -> Result<()> {
        match statement.kind() {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.collect_annex_b_statement_list(statements, var_scope, blocked)
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.collect_annex_b_statement(consequent, var_scope, blocked)?;
                if let Some(alternate) = alternate {
                    self.collect_annex_b_statement(alternate, var_scope, blocked)?;
                }
                Ok(())
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Label { body, .. } => self.collect_annex_b_statement(body, var_scope, blocked),
            Stmt::For { init, body, .. } => {
                let mut body_blocked = blocked.clone();
                if let Some(init) = init {
                    Self::collect_direct_lexical_names(init, &mut body_blocked)?;
                }
                self.collect_annex_b_statement(body, var_scope, &body_blocked)
            }
            Stmt::ForIn { target, body, .. } | Stmt::ForOf { target, body, .. } => {
                let mut body_blocked = blocked.clone();
                Self::collect_for_lexical_names(target, &mut body_blocked)?;
                self.collect_annex_b_statement(body, var_scope, &body_blocked)
            }
            Stmt::Switch { cases, .. } => {
                let statements = cases
                    .iter()
                    .flat_map(|case| case.statements.iter().cloned())
                    .collect::<Vec<_>>();
                self.collect_annex_b_statement_list(&statements, var_scope, blocked)
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_annex_b_statement_list(body, var_scope, blocked)?;
                if let Some(catch) = catch {
                    let mut catch_blocked = blocked.clone();
                    if let Some(pattern) = &catch.param
                        && !matches!(pattern, BindingPattern::Identifier(_))
                    {
                        Self::collect_pattern_names(pattern, &mut catch_blocked)?;
                    }
                    self.collect_annex_b_statement_list(&catch.body, var_scope, &catch_blocked)?;
                }
                if let Some(finally_body) = finally_body {
                    self.collect_annex_b_statement_list(finally_body, var_scope, blocked)?;
                }
                Ok(())
            }
            function @ Stmt::FunctionDecl { .. } => {
                self.collect_annex_b_function(function, var_scope, blocked)
            }
            Stmt::Empty
            | Stmt::Debugger
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::ImportBinding { .. }
            | Stmt::VarDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::Expr(_) => Ok(()),
        }
    }

    fn collect_direct_lexical_names(
        statement: &Statement,
        names: &mut BTreeSet<String>,
    ) -> Result<()> {
        match statement.kind() {
            Stmt::DeclList(statements) => {
                for statement in statements {
                    Self::collect_direct_lexical_names(statement, names)?;
                }
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
            | Stmt::ImportBinding { name }
            | Stmt::ClassDecl { name, .. } => {
                names.insert(name.name().as_str().to_owned());
            }
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            } => Self::collect_pattern_names(pattern, names)?,
            _ => {}
        }
        Ok(())
    }

    fn collect_for_lexical_names(target: &ForInTarget, names: &mut BTreeSet<String>) -> Result<()> {
        match target {
            ForInTarget::Binding {
                name,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            } => {
                names.insert(name.name().as_str().to_owned());
            }
            ForInTarget::PatternBinding {
                pattern,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
            } => Self::collect_pattern_names(pattern, names)?,
            _ => {}
        }
        Ok(())
    }

    fn collect_pattern_names(pattern: &BindingPattern, names: &mut BTreeSet<String>) -> Result<()> {
        pattern.for_each_binding(&mut |binding| {
            names.insert(binding.name().as_str().to_owned());
            Ok(())
        })
    }
}
