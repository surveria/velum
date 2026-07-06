use std::rc::Rc;

use crate::ast::{DeclKind, ForInTarget, StaticBinding, Stmt};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BytecodeHoistPlan {
    var_declarations: Rc<[StaticBinding]>,
}

impl BytecodeHoistPlan {
    pub fn compile(statements: &[Stmt]) -> Self {
        let mut collector = HoistCollector::default();
        collector.collect_statements(statements);
        Self {
            var_declarations: Rc::from(collector.var_declarations.into_boxed_slice()),
        }
    }

    pub fn var_declarations(&self) -> &[StaticBinding] {
        &self.var_declarations
    }

    pub fn var_declaration_count(&self) -> usize {
        self.var_declarations.len()
    }
}

#[derive(Debug, Default)]
struct HoistCollector {
    var_declarations: Vec<StaticBinding>,
}

impl HoistCollector {
    fn collect_statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            self.collect_statement(statement);
        }
    }

    fn collect_statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Block(statements) | Stmt::DeclList(statements) => {
                self.collect_statements(statements);
            }
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                self.collect_statement(consequent);
                if let Some(alternate) = alternate {
                    self.collect_statement(alternate);
                }
            }
            Stmt::While { body, .. } => self.collect_statement(body),
            Stmt::For { init, body, .. } => {
                if let Some(init) = init {
                    self.collect_statement(init);
                }
                self.collect_statement(body);
            }
            Stmt::ForIn { target, body, .. } => {
                if let ForInTarget::Binding {
                    name,
                    kind: DeclKind::Var,
                } = target
                {
                    self.var_declarations.push(name.clone());
                }
                self.collect_statement(body);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    self.collect_statements(&case.statements);
                }
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_statements(body);
                if let Some(catch) = catch {
                    self.collect_statements(&catch.body);
                }
                if let Some(finally_body) = finally_body {
                    self.collect_statements(finally_body);
                }
            }
            Stmt::VarDecl {
                name,
                kind: DeclKind::Var,
                ..
            } => self.var_declarations.push(name.clone()),
            Stmt::Break
            | Stmt::Continue
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::VarDecl { .. }
            | Stmt::Expr(_) => {}
        }
    }
}
