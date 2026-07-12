use std::rc::Rc;

use crate::{
    ast::{BindingPattern, DeclKind, ForInTarget, Statement, StaticBinding, Stmt},
    binding_metadata::BindingLayout,
    bytecode::{BytecodeBinding, BytecodeFunction, BytecodeFunctionDeclaration, BytecodeHoistPlan},
    error::Result,
};

use super::FunctionCompileMode;

impl BytecodeHoistPlan {
    pub fn compile(statements: &[Statement], layout: &BindingLayout) -> Result<Self> {
        let mut collector = HoistCollector::new(layout);
        collector.collect_direct_lexical_declarations(statements);
        collector.collect_statements(statements)?;
        Ok(Self::new(
            Rc::from(collector.lexical_declarations.into_boxed_slice()),
            Rc::from(collector.var_declarations.into_boxed_slice()),
            Rc::from(collector.function_declarations.into_boxed_slice()),
        ))
    }
}

#[derive(Debug)]
struct HoistCollector<'a> {
    layout: &'a BindingLayout,
    lexical_declarations: Vec<(StaticBinding, DeclKind)>,
    var_declarations: Vec<StaticBinding>,
    function_declarations: Vec<BytecodeFunctionDeclaration>,
}

impl<'a> HoistCollector<'a> {
    const fn new(layout: &'a BindingLayout) -> Self {
        Self {
            layout,
            lexical_declarations: Vec::new(),
            var_declarations: Vec::new(),
            function_declarations: Vec::new(),
        }
    }

    fn collect_direct_lexical_declarations(&mut self, statements: &[Statement]) {
        for statement in statements {
            match statement.kind() {
                Stmt::DeclList(declarations) => {
                    self.collect_direct_lexical_declarations(declarations);
                }
                Stmt::VarDecl {
                    name,
                    kind: kind @ (DeclKind::Let | DeclKind::Const),
                    ..
                } => self.lexical_declarations.push((name.clone(), *kind)),
                Stmt::PatternDecl {
                    pattern,
                    kind: kind @ (DeclKind::Let | DeclKind::Const),
                    ..
                } => self.collect_pattern_lexical_declarations(pattern, *kind),
                Stmt::ClassDecl { name, .. } => self
                    .lexical_declarations
                    .push((name.clone(), DeclKind::Let)),
                Stmt::Block(_)
                | Stmt::If { .. }
                | Stmt::While { .. }
                | Stmt::DoWhile { .. }
                | Stmt::Label { .. }
                | Stmt::For { .. }
                | Stmt::ForIn { .. }
                | Stmt::ForOf { .. }
                | Stmt::Switch { .. }
                | Stmt::Try { .. }
                | Stmt::Throw(_)
                | Stmt::Return(_)
                | Stmt::FunctionDecl { .. }
                | Stmt::VarDecl { .. }
                | Stmt::PatternDecl { .. }
                | Stmt::Empty
                | Stmt::Break(_)
                | Stmt::Continue(_)
                | Stmt::Expr(_) => {}
            }
        }
    }

    fn collect_pattern_lexical_declarations(&mut self, pattern: &BindingPattern, kind: DeclKind) {
        let mut visit =
            |binding: &StaticBinding| -> std::result::Result<(), std::convert::Infallible> {
                self.lexical_declarations.push((binding.clone(), kind));
                Ok(())
            };
        match pattern.for_each_binding(&mut visit) {
            Ok(()) => {}
        }
    }

    fn collect_statements(&mut self, statements: &[Statement]) -> Result<()> {
        for statement in statements {
            self.collect_statement(statement)?;
        }
        Ok(())
    }

    fn collect_function_declaration(
        &mut self,
        bindings: (&StaticBinding, Option<&StaticBinding>),
        id: crate::syntax::StaticFunctionId,
        params: &std::rc::Rc<[crate::ast::FunctionParam]>,
        body: &std::rc::Rc<[Statement]>,
        parameter_prologue_count: usize,
        mode: FunctionCompileMode,
    ) -> Result<()> {
        let (name, arguments_binding) = bindings;
        self.var_declarations.push(name.clone());
        let declaration = BytecodeFunctionDeclaration::new(
            BytecodeBinding::compile(name, self.layout)?,
            id,
            name.name().clone(),
            BytecodeFunction::compile(
                None,
                arguments_binding.cloned(),
                params,
                body,
                parameter_prologue_count,
                mode,
                self.layout,
            )?,
            mode.kind,
        );
        self.function_declarations.push(declaration);
        Ok(())
    }

    fn collect_pattern_var_declarations(&mut self, pattern: &BindingPattern) {
        let mut visit =
            |binding: &StaticBinding| -> std::result::Result<(), std::convert::Infallible> {
                self.var_declarations.push(binding.clone());
                Ok(())
            };
        match pattern.for_each_binding(&mut visit) {
            Ok(()) => {}
        }
    }

    fn collect_try_statements(
        &mut self,
        body: &[Statement],
        catch_body: Option<&[Statement]>,
        finally_body: Option<&[Statement]>,
    ) -> Result<()> {
        self.collect_statements(body)?;
        if let Some(catch_body) = catch_body {
            self.collect_statements(catch_body)?;
        }
        if let Some(finally_body) = finally_body {
            self.collect_statements(finally_body)?;
        }
        Ok(())
    }

    fn collect_statement(&mut self, statement: &Statement) -> Result<()> {
        match statement.kind() {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.collect_statements(statements)
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.collect_statement(consequent)?;
                if let Some(alternate) = alternate {
                    self.collect_statement(alternate)?;
                }
                Ok(())
            }
            Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Label { body, .. } => {
                self.collect_statement(body)
            }
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_statement(init)?;
                }
                self.collect_statement(body)?;
                Ok(())
            }
            Stmt::ForIn { target, body, .. } | Stmt::ForOf { target, body, .. } => {
                match target {
                    ForInTarget::Binding {
                        name,
                        kind: DeclKind::Var,
                    } => self.var_declarations.push(name.clone()),
                    ForInTarget::PatternBinding {
                        pattern,
                        kind: DeclKind::Var,
                    } => self.collect_pattern_var_declarations(pattern),
                    ForInTarget::Binding { .. }
                    | ForInTarget::PatternBinding { .. }
                    | ForInTarget::PatternAssignment { .. }
                    | ForInTarget::Assignment(_) => {}
                }
                self.collect_statement(body)
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    self.collect_statements(&case.statements)?;
                }
                Ok(())
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => self.collect_try_statements(
                body,
                catch.as_ref().map(|clause| clause.body.as_ref()),
                finally_body.as_deref(),
            ),
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            } => {
                self.var_declarations.push(name.clone());
                Ok(())
            }
            Stmt::PatternDecl {
                pattern,
                kind: DeclKind::Var,
                ..
            } => {
                self.collect_pattern_var_declarations(pattern);
                Ok(())
            }
            Stmt::FunctionDecl {
                name,
                arguments_binding,
                id,
                params,
                body,
                parameter_prologue_count,
                kind,
                strict,
            } => self.collect_function_declaration(
                (name, arguments_binding.as_ref()),
                *id,
                params,
                body,
                *parameter_prologue_count,
                FunctionCompileMode::new(*kind, *strict),
            ),
            Stmt::Empty
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => Ok(()),
        }
    }
}
