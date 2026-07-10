use std::rc::Rc;

use crate::{
    ast::{BindingPattern, DeclKind, ForInTarget, Statement, StaticBinding, Stmt},
    binding_metadata::BindingLayout,
    bytecode::{BytecodeBinding, BytecodeFunction, BytecodeFunctionDeclaration, BytecodeHoistPlan},
    error::Result,
};

impl BytecodeHoistPlan {
    pub fn compile(statements: &[Statement], layout: &BindingLayout) -> Result<Self> {
        let mut collector = HoistCollector::new(layout);
        collector.collect_statements(statements)?;
        Ok(Self::new(
            Rc::from(collector.var_declarations.into_boxed_slice()),
            Rc::from(collector.function_declarations.into_boxed_slice()),
        ))
    }
}

#[derive(Debug)]
struct HoistCollector<'a> {
    layout: &'a BindingLayout,
    var_declarations: Vec<StaticBinding>,
    function_declarations: Vec<BytecodeFunctionDeclaration>,
}

impl<'a> HoistCollector<'a> {
    const fn new(layout: &'a BindingLayout) -> Self {
        Self {
            layout,
            var_declarations: Vec::new(),
            function_declarations: Vec::new(),
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
        name: &StaticBinding,
        id: crate::syntax::StaticFunctionId,
        params: &std::rc::Rc<[crate::ast::FunctionParam]>,
        body: &std::rc::Rc<[Statement]>,
        is_async: bool,
    ) -> Result<()> {
        self.var_declarations.push(name.clone());
        let declaration = BytecodeFunctionDeclaration::new(
            BytecodeBinding::compile(name, self.layout)?,
            id,
            name.name().clone(),
            BytecodeFunction::compile(params, body, self.layout)?,
            is_async,
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
            } => {
                self.collect_statements(body)?;
                if let Some(catch) = catch {
                    self.collect_statements(&catch.body)?;
                }
                if let Some(finally_body) = finally_body {
                    self.collect_statements(finally_body)?;
                }
                Ok(())
            }
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
                id,
                params,
                body,
                is_async,
            } => self.collect_function_declaration(name, *id, params, body, *is_async),
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
