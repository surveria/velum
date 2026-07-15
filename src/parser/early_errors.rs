use std::collections::BTreeSet;

use crate::{
    ast::{BindingPattern, DeclKind, ForInTarget, FunctionParam, Statement, Stmt, SwitchCase},
    error::Result,
    syntax::FunctionKind,
};

use super::Parser;

impl Parser {
    pub(super) fn validate_script_declarations(&self, statements: &[Statement]) -> Result<()> {
        self.validate_lexical_var_declarations(statements, false, false)
    }

    pub(super) fn validate_module_declarations(&self, statements: &[Statement]) -> Result<()> {
        self.validate_lexical_var_declarations(statements, true, false)
    }

    pub(super) fn validate_for_in_of_declarations(
        &self,
        target: &ForInTarget,
        body: &Statement,
    ) -> Result<()> {
        let mut lexical_names = Vec::new();
        match target {
            ForInTarget::Binding { name, kind, .. } if *kind != DeclKind::Var => {
                lexical_names.push(name.name().as_str().to_owned());
            }
            ForInTarget::PatternBinding { pattern, kind } if *kind != DeclKind::Var => {
                Self::collect_pattern_names(pattern, &mut lexical_names)?;
            }
            ForInTarget::Binding { .. }
            | ForInTarget::PatternBinding { .. }
            | ForInTarget::PatternAssignment { .. }
            | ForInTarget::Assignment { .. } => {}
        }
        self.validate_loop_head_names(&lexical_names, body)
    }

    pub(super) fn validate_for_declarations(
        &self,
        init: Option<&Statement>,
        body: &Statement,
    ) -> Result<()> {
        let mut lexical_names = Vec::new();
        if let Some(init) = init {
            Self::collect_direct_lexical_names(init, false, &mut lexical_names)?;
        }
        self.validate_loop_head_names(&lexical_names, body)
    }

    fn validate_loop_head_names(&self, lexical_names: &[String], body: &Statement) -> Result<()> {
        let mut unique_names = BTreeSet::new();
        for name in lexical_names {
            if !unique_names.insert(name) {
                return Err(self.parse_error("duplicate lexical declaration in for head"));
            }
        }
        let mut var_names = Vec::new();
        Self::collect_var_names(body, &mut var_names)?;
        if lexical_names
            .iter()
            .any(|name| var_names.iter().any(|var_name| var_name == name))
        {
            return Err(self.parse_error("for head declaration conflicts with a body var"));
        }
        Ok(())
    }

    pub(super) fn reject_invalid_single_statement(&self, statement: &Statement) -> Result<()> {
        if matches!(statement.kind(), Stmt::ClassDecl { .. })
            || matches!(
                statement.kind(),
                Stmt::FunctionDecl { kind, .. }
                    if self.is_strict_mode() || *kind != FunctionKind::Ordinary
            )
            || matches!(
                statement.kind(),
                Stmt::VarDecl {
                    kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                    ..
                } | Stmt::PatternDecl {
                    kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                    ..
                }
            )
        {
            return Err(self.parse_error("declaration is not allowed as a single statement body"));
        }
        Ok(())
    }

    pub(super) fn reject_invalid_iteration_statement(&self, statement: &Statement) -> Result<()> {
        self.reject_invalid_single_statement(statement)?;
        if Self::is_labelled_function(statement) {
            return Err(self.parse_error("function declaration is not allowed as a loop body"));
        }
        Ok(())
    }

    pub(super) fn reject_invalid_if_statement(&self, statement: &Statement) -> Result<()> {
        self.reject_invalid_single_statement(statement)?;
        if matches!(statement.kind(), Stmt::Label { .. }) && Self::is_labelled_function(statement) {
            return Err(
                self.parse_error("labelled function declaration is not allowed as an if body")
            );
        }
        Ok(())
    }

    fn is_labelled_function(statement: &Statement) -> bool {
        match statement.kind() {
            Stmt::FunctionDecl { .. } => true,
            Stmt::Label { body, .. } => Self::is_labelled_function(body),
            _ => false,
        }
    }

    pub(super) fn validate_generator_block_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<()> {
        self.validate_lexical_var_declarations(statements, true, true)
    }

    pub(super) fn validate_static_block_declarations(
        &self,
        statements: &[Statement],
    ) -> Result<()> {
        self.validate_lexical_var_declarations(statements, true, true)
    }

    pub(super) fn validate_switch_declarations(&self, cases: &[SwitchCase]) -> Result<()> {
        let statements = cases
            .iter()
            .flat_map(|case| case.statements.iter())
            .collect::<Vec<_>>();
        self.validate_statement_refs(&statements, true, true)
    }

    pub(super) fn validate_function_parameter_lexicals(
        &self,
        params: &[FunctionParam],
        body: &[Statement],
    ) -> Result<()> {
        let mut parameter_names = BTreeSet::new();
        for param in params {
            param.target.for_each_binding(&mut |binding| {
                parameter_names.insert(binding.name().as_str().to_owned());
                Ok::<(), crate::Error>(())
            })?;
        }
        for statement in body {
            let mut lexical_names = Vec::new();
            Self::collect_direct_lexical_names(statement, false, &mut lexical_names)?;
            if lexical_names
                .iter()
                .any(|name| parameter_names.contains(name))
            {
                return Err(self
                    .parse_error("function parameter conflicts with a lexical body declaration"));
            }
        }
        Ok(())
    }

    pub(super) fn validate_catch_parameter_lexicals(
        &self,
        param: &BindingPattern,
        body: &[Statement],
    ) -> Result<()> {
        let mut parameter_names = BTreeSet::new();
        param.for_each_binding(&mut |binding| {
            parameter_names.insert(binding.name().as_str().to_owned());
            Ok::<(), crate::Error>(())
        })?;
        let mut lexical_names = Vec::new();
        for statement in body {
            Self::collect_direct_lexical_names(statement, true, &mut lexical_names)?;
        }
        if lexical_names
            .iter()
            .any(|name| parameter_names.contains(name))
        {
            return Err(
                self.parse_error("catch parameter conflicts with a lexical body declaration")
            );
        }
        Ok(())
    }

    fn validate_lexical_var_declarations(
        &self,
        statements: &[Statement],
        functions_are_lexical: bool,
        allow_duplicate_sloppy_functions: bool,
    ) -> Result<()> {
        let statements = statements.iter().collect::<Vec<_>>();
        self.validate_statement_refs(
            &statements,
            functions_are_lexical,
            allow_duplicate_sloppy_functions,
        )
    }

    fn validate_statement_refs(
        &self,
        statements: &[&Statement],
        functions_are_lexical: bool,
        allow_duplicate_sloppy_functions: bool,
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
            if !unique_lexical_names.insert(name.clone())
                && !self.allows_duplicate_sloppy_block_functions(
                    statements,
                    functions_are_lexical,
                    allow_duplicate_sloppy_functions,
                    &name,
                )?
            {
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

    fn allows_duplicate_sloppy_block_functions(
        &self,
        statements: &[&Statement],
        functions_are_lexical: bool,
        allow_duplicate_sloppy_functions: bool,
        name: &str,
    ) -> Result<bool> {
        if self.is_strict_mode() || !functions_are_lexical || !allow_duplicate_sloppy_functions {
            return Ok(false);
        }
        let mut function_count = 0usize;
        for statement in statements {
            if !Self::direct_name_is_only_function(statement, name, &mut function_count)? {
                return Ok(false);
            }
        }
        Ok(function_count >= 2)
    }

    fn direct_name_is_only_function(
        statement: &Statement,
        name: &str,
        function_count: &mut usize,
    ) -> Result<bool> {
        match statement.kind() {
            Stmt::DeclList(declarations) => {
                for declaration in declarations {
                    if !Self::direct_name_is_only_function(declaration, name, function_count)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Stmt::FunctionDecl {
                name: function_name,
                kind: FunctionKind::Ordinary,
                ..
            } if function_name.name().as_str() == name => {
                *function_count = function_count
                    .checked_add(1)
                    .ok_or_else(|| crate::error::Error::limit("function count overflowed"))?;
                Ok(true)
            }
            Stmt::VarDecl {
                name: binding,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
            | Stmt::ImportBinding { name: binding }
            | Stmt::ClassDecl { name: binding, .. } => Ok(binding.name().as_str() != name),
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            } => {
                let mut names = Vec::new();
                Self::collect_pattern_names(pattern, &mut names)?;
                Ok(!names.iter().any(|candidate| candidate == name))
            }
            _ => Ok(true),
        }
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
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            }
            | Stmt::ImportBinding { name }
            | Stmt::ClassDecl { name, .. } => names.push(name.name().as_str().to_owned()),
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing,
                ..
            } => Self::collect_pattern_names(pattern, names)?,
            Stmt::FunctionDecl { name, .. } if functions_are_lexical => {
                names.push(name.name().as_str().to_owned());
            }
            Stmt::Empty
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
                        ..
                    } => names.push(name.name().as_str().to_owned()),
                    crate::ast::ForInTarget::PatternBinding {
                        pattern,
                        kind: DeclKind::Var,
                    } => Self::collect_pattern_names(pattern, names)?,
                    crate::ast::ForInTarget::Binding { .. }
                    | crate::ast::ForInTarget::PatternBinding { .. }
                    | crate::ast::ForInTarget::PatternAssignment { .. }
                    | crate::ast::ForInTarget::Assignment { .. } => {}
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
            | Stmt::Debugger
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::FunctionDecl { .. }
            | Stmt::ImportBinding { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::VarDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::Expr(_) => {}
        }
        Ok(())
    }

    pub(super) fn collect_pattern_names(
        pattern: &BindingPattern,
        names: &mut Vec<String>,
    ) -> Result<()> {
        pattern.for_each_binding(&mut |binding| {
            names.push(binding.name().as_str().to_owned());
            Ok(())
        })
    }
}
