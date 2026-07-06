use std::rc::Rc;

use crate::{
    ast::{CatchClause, Expr, ForInTarget, ObjectProperty, StaticBinding, Stmt, SwitchCase},
    binding_layout::BindingLayout,
    error::Result,
};

use super::{BytecodeBlock, BytecodeFunction, BytecodeHoistPlan, StatementValue};

impl BytecodeFunction {
    pub fn compile(statements: &[Stmt], layout: &BindingLayout) -> Result<Self> {
        Ok(Self::new(
            BytecodeBlock::compile_statements(statements, StatementValue::Store, layout)?,
            BytecodeHoistPlan::compile(statements, layout)?,
            CaptureBindingCollector::collect(statements),
        ))
    }
}

#[derive(Debug, Default)]
struct CaptureBindingCollector {
    bindings: Vec<StaticBinding>,
}

impl CaptureBindingCollector {
    fn collect(statements: &[Stmt]) -> Rc<[StaticBinding]> {
        let mut collector = Self::default();
        collector.collect_statements(statements);
        Rc::from(collector.bindings.into_boxed_slice())
    }

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
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition);
                self.collect_statement(consequent);
                if let Some(alternate) = alternate {
                    self.collect_statement(alternate);
                }
            }
            Stmt::While { condition, body } => {
                self.collect_expr(condition);
                self.collect_statement(body);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_statement(init);
                }
                if let Some(condition) = condition {
                    self.collect_expr(condition);
                }
                if let Some(update) = update {
                    self.collect_expr(update);
                }
                self.collect_statement(body);
            }
            Stmt::ForIn {
                target,
                object,
                body,
            } => {
                self.collect_for_in_target(target);
                self.collect_expr(object);
                self.collect_statement(body);
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                self.collect_expr(discriminant);
                self.collect_switch_cases(cases);
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_statements(body);
                if let Some(catch) = catch {
                    self.collect_catch(catch);
                }
                if let Some(finally_body) = finally_body {
                    self.collect_statements(finally_body);
                }
            }
            Stmt::Throw(expr) | Stmt::Expr(expr) => self.collect_expr(expr),
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    self.collect_expr(expr);
                }
            }
            Stmt::FunctionDecl { body, .. } => self.collect_statements(body),
            Stmt::VarDecl { init, .. } => {
                if let Some(init) = init {
                    self.collect_expr(init);
                }
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }

    fn collect_for_in_target(&mut self, target: &ForInTarget) {
        match target {
            ForInTarget::Binding { .. } => {}
            ForInTarget::Assignment(expr) => self.collect_expr(expr),
        }
    }

    fn collect_switch_cases(&mut self, cases: &[SwitchCase]) {
        for case in cases {
            if let Some(test) = &case.test {
                self.collect_expr(test);
            }
            self.collect_statements(&case.statements);
        }
    }

    fn collect_catch(&mut self, catch: &CatchClause) {
        self.collect_statements(&catch.body);
    }

    fn collect_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) | Expr::StringLiteral(_) | Expr::This => {}
            Expr::Function { body, .. }
            | Expr::ArrowFunction { body, .. }
            | Expr::MethodFunction { body, .. } => {
                self.collect_statements(body);
            }
            Expr::Identifier(binding)
            | Expr::New {
                constructor: binding,
                ..
            } => {
                self.collect_binding(binding);
                if let Expr::New { args, .. } = expr {
                    self.collect_exprs(args);
                }
            }
            Expr::Parenthesized(expr)
            | Expr::Unary { expr, .. }
            | Expr::Update { expr, .. }
            | Expr::Await(expr) => {
                self.collect_expr(expr);
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr(left);
                self.collect_expr(right);
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition);
                self.collect_expr(consequent);
                self.collect_expr(alternate);
            }
            Expr::Assignment { name, expr } => {
                self.collect_binding(name);
                self.collect_expr(expr);
            }
            Expr::CompoundAssignment { target, expr, .. } => {
                self.collect_expr(target);
                self.collect_expr(expr);
            }
            Expr::PropertyAssignment { object, expr, .. } => {
                self.collect_expr(object);
                self.collect_expr(expr);
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
                ..
            } => {
                self.collect_expr(object);
                self.collect_expr(property);
                self.collect_expr(expr);
            }
            Expr::Member { object, .. } => self.collect_expr(object),
            Expr::ComputedMember {
                object, property, ..
            } => {
                self.collect_expr(object);
                self.collect_expr(property);
            }
            Expr::Call { callee, args } => {
                self.collect_expr(callee);
                self.collect_exprs(args);
            }
            Expr::Object(properties) => self.collect_object_properties(properties),
            Expr::Array(elements) => self.collect_exprs(elements),
        }
    }

    fn collect_object_properties(&mut self, properties: &[ObjectProperty]) {
        for property in properties {
            self.collect_expr(&property.value);
        }
    }

    fn collect_exprs(&mut self, exprs: &[Expr]) {
        for expr in exprs {
            self.collect_expr(expr);
        }
    }

    fn collect_binding(&mut self, binding: &StaticBinding) {
        if self
            .bindings
            .iter()
            .any(|existing| existing.id() == binding.id())
        {
            return;
        }
        self.bindings.push(binding.clone());
    }
}
