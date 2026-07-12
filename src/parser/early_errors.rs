use std::collections::BTreeSet;

use crate::{
    ast::{BindingPattern, DeclKind, FunctionParam, Statement, Stmt, SwitchCase},
    error::Result,
};

use super::Parser;

impl Parser {
    pub(super) fn reject_invalid_single_statement(&self, statement: &Statement) -> Result<()> {
        if matches!(
            statement.kind(),
            Stmt::FunctionDecl { kind, .. } if kind.is_generator()
        ) || matches!(
            statement.kind(),
            Stmt::VarDecl {
                kind: DeclKind::Let | DeclKind::Const,
                ..
            } | Stmt::PatternDecl {
                kind: DeclKind::Let | DeclKind::Const,
                ..
            }
        ) {
            return Err(self.parse_error("declaration is not allowed as a single statement body"));
        }
        Ok(())
    }

    pub(super) fn validate_generator_block_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<()> {
        if !statements.iter().any(|statement| {
            matches!(
                statement.kind(),
                Stmt::FunctionDecl { kind, .. } if kind.is_generator()
            )
        }) {
            return Ok(());
        }
        self.validate_lexical_var_declarations(statements, true)
    }

    pub(super) fn validate_static_block_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<()> {
        self.validate_lexical_var_declarations(statements, true)
    }

    pub(super) fn validate_generator_switch_declarations(
        &self,
        cases: &[SwitchCase],
    ) -> Result<()> {
        let statements = cases
            .iter()
            .flat_map(|case| case.statements.iter())
            .collect::<Vec<_>>();
        if !statements.iter().any(|statement| {
            matches!(
                statement.kind(),
                Stmt::FunctionDecl { kind, .. } if kind.is_generator()
            )
        }) {
            return Ok(());
        }
        self.validate_statement_refs(&statements, true)
    }

    pub(super) fn validate_generator_parameter_lexicals(
        &self,
        params: &[FunctionParam],
        body: &[Statement],
    ) -> Result<()> {
        let mut parameter_names = BTreeSet::new();
        for param in params {
            parameter_names.insert(param.name.name().as_str().to_owned());
        }
        for statement in body {
            let mut lexical_names = Vec::new();
            Self::collect_direct_lexical_names(statement, false, &mut lexical_names)?;
            if lexical_names
                .iter()
                .any(|name| parameter_names.contains(name))
            {
                return Err(self
                    .parse_error("generator parameter conflicts with a lexical body declaration"));
            }
        }
        Ok(())
    }

    fn validate_lexical_var_declarations(
        &self,
        statements: &[Statement],
        functions_are_lexical: bool,
    ) -> Result<()> {
        let statements = statements.iter().collect::<Vec<_>>();
        self.validate_statement_refs(&statements, functions_are_lexical)
    }

    fn validate_statement_refs(
        &self,
        statements: &[&Statement],
        functions_are_lexical: bool,
    ) -> Result<()> {
        let mut lexical_names = Vec::new();
        for statement in statements {
            Self::collect_direct_lexical_names(
                statement,
                functions_are_lexical,
                &mut lexical_names,
            )?;
        }
        let mut unique_lexical_names = BTreeSet::new();
        for name in lexical_names {
            if !unique_lexical_names.insert(name) {
                return Err(self.parse_error("duplicate lexical declaration"));
            }
        }

        let mut var_names = Vec::new();
        for statement in statements {
            Self::collect_var_names(statement, &mut var_names)?;
        }
        if var_names
            .iter()
            .any(|name| unique_lexical_names.contains(name))
        {
            return Err(self.parse_error("lexical declaration conflicts with a var declaration"));
        }
        Ok(())
    }

    fn collect_direct_lexical_names(
        statement: &Statement,
        functions_are_lexical: bool,
        names: &mut Vec<String>,
    ) -> Result<()> {
        match statement.kind() {
            Stmt::DeclList(declarations) => {
                for declaration in declarations {
                    Self::collect_direct_lexical_names(declaration, functions_are_lexical, names)?;
                }
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Let | DeclKind::Const,
                ..
            }
            | Stmt::ClassDecl { name, .. } => names.push(name.name().as_str().to_owned()),
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Let | DeclKind::Const,
                ..
            } => Self::collect_pattern_names(pattern, names)?,
            Stmt::FunctionDecl { name, .. } if functions_are_lexical => {
                names.push(name.name().as_str().to_owned());
            }
            Stmt::Empty
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
            | Stmt::FunctionDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => {}
        }
        Ok(())
    }

    fn collect_var_names(statement: &Statement, names: &mut Vec<String>) -> Result<()> {
        match statement.kind() {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                for statement in statements {
                    Self::collect_var_names(statement, names)?;
                }
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                Self::collect_var_names(consequent, names)?;
                if let Some(alternate) = alternate {
                    Self::collect_var_names(alternate, names)?;
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::With { body, .. }
            | Stmt::Label { body, .. } => {
                Self::collect_var_names(body, names)?;
            }
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    Self::collect_var_names(init, names)?;
                }
                Self::collect_var_names(body, names)?;
            }
            Stmt::ForIn { target, body, .. } | Stmt::ForOf { target, body, .. } => {
                match target {
                    crate::ast::ForInTarget::Binding {
                        name,
                        kind: DeclKind::Var,
                    } => names.push(name.name().as_str().to_owned()),
                    crate::ast::ForInTarget::PatternBinding {
                        pattern,
                        kind: DeclKind::Var,
                    } => Self::collect_pattern_names(pattern, names)?,
                    crate::ast::ForInTarget::Binding { .. }
                    | crate::ast::ForInTarget::PatternBinding { .. }
                    | crate::ast::ForInTarget::PatternAssignment { .. }
                    | crate::ast::ForInTarget::Assignment(_) => {}
                }
                Self::collect_var_names(body, names)?;
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    for statement in &case.statements {
                        Self::collect_var_names(statement, names)?;
                    }
                }
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                for statement in body {
                    Self::collect_var_names(statement, names)?;
                }
                if let Some(catch) = catch {
                    for statement in &catch.body {
                        Self::collect_var_names(statement, names)?;
                    }
                }
                if let Some(finally_body) = finally_body {
                    for statement in finally_body {
                        Self::collect_var_names(statement, names)?;
                    }
                }
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            } => names.push(name.name().as_str().to_owned()),
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Var,
                ..
            } => Self::collect_pattern_names(pattern, names)?,
            Stmt::Empty
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::FunctionDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::VarDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::Expr(_) => {}
        }
        Ok(())
    }

    fn collect_pattern_names(pattern: &BindingPattern, names: &mut Vec<String>) -> Result<()> {
        pattern.for_each_binding(&mut |binding| {
            names.push(binding.name().as_str().to_owned());
            Ok(())
        })
    }
}
