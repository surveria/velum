use std::rc::Rc;

use crate::{
    ast::{DeclKind, ForInTarget, StaticBinding, Stmt},
    binding_layout::BindingLayout,
    bytecode::{BytecodeBinding, BytecodeFunction, BytecodeFunctionDeclaration},
    error::Result,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeHoistPlan {
    var_declarations: Rc<[StaticBinding]>,
    function_declarations: Rc<[BytecodeFunctionDeclaration]>,
}

impl BytecodeHoistPlan {
    pub fn compile(statements: &[Stmt], layout: &BindingLayout) -> Result<Self> {
        let mut collector = HoistCollector::new(layout);
        collector.collect_statements(statements)?;
        Ok(Self {
            var_declarations: Rc::from(collector.var_declarations.into_boxed_slice()),
            function_declarations: Rc::from(collector.function_declarations.into_boxed_slice()),
        })
    }

    pub fn var_declarations(&self) -> &[StaticBinding] {
        &self.var_declarations
    }

    pub fn function_declarations(&self) -> &[BytecodeFunctionDeclaration] {
        &self.function_declarations
    }

    pub fn var_declaration_count(&self) -> usize {
        self.var_declarations.len()
    }

    pub fn function_declaration_count(&self) -> usize {
        self.function_declarations.len()
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

    fn collect_statements(&mut self, statements: &[Stmt]) -> Result<()> {
        for statement in statements {
            self.collect_statement(statement)?;
        }
        Ok(())
    }

    fn collect_statement(&mut self, statement: &Stmt) -> Result<()> {
        match statement {
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
            Stmt::ForIn { target, body, .. } => {
                if let ForInTarget::Binding {
                    name,
                    kind: DeclKind::Var,
                } = target
                {
                    self.var_declarations.push(name.clone());
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
            Stmt::FunctionDecl {
                name,
                id,
                params,
                body,
                is_async,
            } => {
                self.var_declarations.push(name.clone());
                let declaration = BytecodeFunctionDeclaration::new(
                    BytecodeBinding::compile(name, self.layout)?,
                    *id,
                    name.name().clone(),
                    BytecodeFunction::compile(params, body, self.layout)?,
                    *is_async,
                );
                self.function_declarations.push(declaration);
                Ok(())
            }
            Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => Ok(()),
        }
    }
}
