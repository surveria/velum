use std::rc::Rc;

use crate::{
    ast::{CatchClause, Expr, ForInTarget, StaticBinding, Stmt, SwitchCase},
    binding_layout::BindingLayout,
    binding_layout_types::BindingOperand,
    error::{Error, Result},
    runtime::Context,
    runtime_scope::BindingCell,
};

impl Context {
    pub(crate) fn capture_function_upvalues(
        &self,
        body: &[Stmt],
        layout: Option<&BindingLayout>,
    ) -> Result<super::FunctionUpvalues> {
        let Some(layout) = layout else {
            return Ok(Rc::from(Vec::new().into_boxed_slice()));
        };
        let mut collector = UpvalueCollector::new(self, layout);
        collector.collect_statements(body)?;
        Ok(Rc::from(collector.into_cells().into_boxed_slice()))
    }
}

struct UpvalueCollector<'a> {
    context: &'a Context,
    layout: &'a BindingLayout,
    cells: Vec<Option<BindingCell>>,
}

impl<'a> UpvalueCollector<'a> {
    const fn new(context: &'a Context, layout: &'a BindingLayout) -> Self {
        Self {
            context,
            layout,
            cells: Vec::new(),
        }
    }

    fn into_cells(self) -> Vec<Option<BindingCell>> {
        self.cells
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
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition)?;
                self.collect_statement(consequent)?;
                if let Some(alternate) = alternate {
                    self.collect_statement(alternate)?;
                }
                Ok(())
            }
            Stmt::While { condition, body } => {
                self.collect_expr(condition)?;
                self.collect_statement(body)
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.collect_statement(init)?;
                }
                if let Some(condition) = condition {
                    self.collect_expr(condition)?;
                }
                if let Some(update) = update {
                    self.collect_expr(update)?;
                }
                self.collect_statement(body)
            }
            Stmt::ForIn {
                target,
                object,
                body,
            } => {
                self.collect_for_in_target(target)?;
                self.collect_expr(object)?;
                self.collect_statement(body)
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                self.collect_expr(discriminant)?;
                self.collect_switch_cases(cases)
            }
            Stmt::Try {
                body,
                catch,
                finally_body,
            } => {
                self.collect_statements(body)?;
                if let Some(catch) = catch {
                    self.collect_catch(catch)?;
                }
                if let Some(finally_body) = finally_body {
                    self.collect_statements(finally_body)?;
                }
                Ok(())
            }
            Stmt::Throw(expr) | Stmt::Expr(expr) => self.collect_expr(expr),
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    return self.collect_expr(expr);
                }
                Ok(())
            }
            Stmt::VarDecl { init, .. } => {
                if let Some(init) = init {
                    return self.collect_expr(init);
                }
                Ok(())
            }
            Stmt::Break | Stmt::Continue => Ok(()),
        }
    }

    fn collect_for_in_target(&mut self, target: &ForInTarget) -> Result<()> {
        match target {
            ForInTarget::Binding { .. } => Ok(()),
            ForInTarget::Assignment(expr) => self.collect_expr(expr),
        }
    }

    fn collect_switch_cases(&mut self, cases: &[SwitchCase]) -> Result<()> {
        for case in cases {
            if let Some(test) = &case.test {
                self.collect_expr(test)?;
            }
            self.collect_statements(&case.statements)?;
        }
        Ok(())
    }

    fn collect_catch(&mut self, catch: &CatchClause) -> Result<()> {
        self.collect_statements(&catch.body)
    }

    fn collect_expr(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::Literal(_) | Expr::This | Expr::Function { .. } | Expr::MethodFunction { .. } => {
                Ok(())
            }
            Expr::Identifier(binding)
            | Expr::New {
                constructor: binding,
                ..
            } => {
                self.capture_binding(binding)?;
                if let Expr::New { args, .. } = expr {
                    return self.collect_exprs(args);
                }
                Ok(())
            }
            Expr::Parenthesized(expr) | Expr::Unary { expr, .. } | Expr::Update { expr, .. } => {
                self.collect_expr(expr)
            }
            Expr::Binary { left, right, .. } => {
                self.collect_expr(left)?;
                self.collect_expr(right)
            }
            Expr::Conditional {
                condition,
                consequent,
                alternate,
            } => {
                self.collect_expr(condition)?;
                self.collect_expr(consequent)?;
                self.collect_expr(alternate)
            }
            Expr::Assignment { name, expr } => {
                self.capture_binding(name)?;
                self.collect_expr(expr)
            }
            Expr::CompoundAssignment { target, expr, .. } => {
                self.collect_expr(target)?;
                self.collect_expr(expr)
            }
            Expr::PropertyAssignment { object, expr, .. } => {
                self.collect_expr(object)?;
                self.collect_expr(expr)
            }
            Expr::ComputedPropertyAssignment {
                object,
                property,
                expr,
            } => {
                self.collect_expr(object)?;
                self.collect_expr(property)?;
                self.collect_expr(expr)
            }
            Expr::Member { object, .. } => self.collect_expr(object),
            Expr::ComputedMember { object, property } => {
                self.collect_expr(object)?;
                self.collect_expr(property)
            }
            Expr::Call { callee, args } => {
                self.collect_expr(callee)?;
                self.collect_exprs(args)
            }
            Expr::Object(properties) => {
                for property in properties {
                    self.collect_expr(&property.value)?;
                }
                Ok(())
            }
            Expr::Array(elements) => self.collect_exprs(elements),
        }
    }

    fn collect_exprs(&mut self, exprs: &[Expr]) -> Result<()> {
        for expr in exprs {
            self.collect_expr(expr)?;
        }
        Ok(())
    }

    fn capture_binding(&mut self, binding: &StaticBinding) -> Result<()> {
        let Some(BindingOperand::Upvalue { slot, .. }) =
            self.layout.operand_for_binding_id(binding.id())?
        else {
            return Ok(());
        };
        let index = slot.index()?;
        self.ensure_cell_slot(index);
        let Some(target) = self.cells.get_mut(index) else {
            return Err(Error::runtime("upvalue cell slot is not defined"));
        };
        if target.is_some() {
            return Ok(());
        }
        *target = self.context.resolve_runtime_static_binding(binding)?;
        Ok(())
    }

    fn ensure_cell_slot(&mut self, index: usize) {
        let required_len = index.saturating_add(1);
        self.cells.resize_with(required_len, || None);
    }
}
