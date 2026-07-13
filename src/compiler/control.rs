use std::rc::Rc;

use crate::{
    ast::{CatchClause, Expr, Expression, ForInTarget, Statement, Stmt, SwitchCase},
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock, BytecodeCatch,
        BytecodeDirectThrow, BytecodeForInTarget, BytecodeInstruction, BytecodeSwitchCase,
    },
    error::{Error, Result},
    syntax::StaticName,
};

use super::{BytecodeCompiler, StatementValue, statements_need_lexical_scope};

impl BytecodeCompiler<'_> {
    pub(super) fn compile_with(
        &mut self,
        object: &Expression,
        body: &Statement,
        value: StatementValue,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.emit(BytecodeInstruction::With {
            body: self.compile_statement_block(body, value)?,
        });
        Ok(())
    }

    pub(super) fn compile_if(
        &mut self,
        condition: &Expression,
        consequent: &Statement,
        alternate: Option<&Statement>,
        value: StatementValue,
    ) -> Result<()> {
        self.compile_expr(condition)?;
        let alternate_jump = self.emit_jump_if_false();
        self.compile_statement(consequent, value)?;
        self.emit_discard_branch_last(value);
        let end_jump = self.emit_jump();
        let alternate_address = self.current_address();
        self.patch_jump(alternate_jump, alternate_address)?;
        if let Some(alternate) = alternate {
            self.compile_statement(alternate, value)?;
            self.emit_discard_branch_last(value);
        } else {
            self.emit_undefined_last();
        }
        let end_address = self.current_address();
        self.patch_jump(end_jump, end_address)
    }

    pub(super) fn compile_while(&mut self, condition: &Expression, body: &Statement) -> Result<()> {
        self.compile_labeled_while(None, condition, body)
    }

    fn compile_labeled_while(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        condition: &Expression,
        body: &Statement,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::While {
            labels,
            condition: BytecodeBlock::compile_expression(condition, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    pub(super) fn compile_do_while(
        &mut self,
        body: &Statement,
        condition: &Expression,
    ) -> Result<()> {
        self.compile_labeled_do_while(None, body, condition)
    }

    fn compile_labeled_do_while(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        body: &Statement,
        condition: &Expression,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::DoWhile {
            labels,
            body: self.compile_statement_block(body, StatementValue::Store)?,
            condition: BytecodeBlock::compile_expression(condition, self.layout)?,
        });
        Ok(())
    }

    pub(super) fn compile_label(
        &mut self,
        label: &StaticName,
        body: &Statement,
        value: StatementValue,
    ) -> Result<()> {
        let (labels, labeled_body) = collect_label_chain(label, body);
        let labels = Some(labels);
        match labeled_body.kind() {
            Stmt::While { condition, body } => {
                return self.compile_labeled_while(labels, condition, body);
            }
            Stmt::DoWhile { body, condition } => {
                return self.compile_labeled_do_while(labels, body, condition);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                return self.compile_labeled_for(
                    labels,
                    init.as_deref(),
                    condition.as_ref(),
                    update.as_ref(),
                    body,
                );
            }
            Stmt::ForIn {
                target,
                object,
                body,
            } => {
                return self.compile_labeled_for_in(labels, target, object, body);
            }
            Stmt::ForOf {
                target,
                object,
                body,
                asynchronous,
            } => {
                return self.compile_labeled_for_of(labels, target, object, body, *asynchronous);
            }
            Stmt::Block(_)
            | Stmt::DeclList(_)
            | Stmt::Empty
            | Stmt::Debugger
            | Stmt::If { .. }
            | Stmt::With { .. }
            | Stmt::Label { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::FunctionDecl { .. }
            | Stmt::ImportBinding { .. }
            | Stmt::VarDecl { .. }
            | Stmt::PatternDecl { .. }
            | Stmt::ClassDecl { .. }
            | Stmt::Expr(_) => {}
        }
        self.emit(BytecodeInstruction::Label {
            label: label.clone(),
            body: self.compile_statement_block(body, value)?,
        });
        Ok(())
    }

    pub(super) fn compile_for(
        &mut self,
        init: Option<&Statement>,
        condition: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
    ) -> Result<()> {
        self.compile_labeled_for(None, init, condition, update, body)
    }

    fn compile_labeled_for(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        init: Option<&Statement>,
        condition: Option<&Expression>,
        update: Option<&Expression>,
        body: &Statement,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::For {
            labels,
            init: init
                .map(|init| self.compile_statement_block(init, StatementValue::Discard))
                .transpose()?,
            condition: condition
                .map(|condition| BytecodeBlock::compile_expression(condition, self.layout))
                .transpose()?,
            update: update
                .map(|update| BytecodeBlock::compile_expression(update, self.layout))
                .transpose()?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
            scoped: crate::binding_layout::for_init_needs_lexical_scope(init),
            per_iteration: crate::binding_layout::for_init_needs_per_iteration_scope(init),
        });
        Ok(())
    }

    pub(super) fn compile_for_in(
        &mut self,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
    ) -> Result<()> {
        self.compile_labeled_for_in(None, target, object, body)
    }

    fn compile_labeled_for_in(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::ForIn {
            labels,
            target: self.compile_for_in_target(target)?,
            object: BytecodeBlock::compile_expression(object, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    pub(super) fn compile_for_of(
        &mut self,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
        asynchronous: bool,
    ) -> Result<()> {
        self.compile_labeled_for_of(None, target, object, body, asynchronous)
    }

    fn compile_labeled_for_of(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
        asynchronous: bool,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::ForOf {
            labels,
            target: self.compile_for_in_target(target)?,
            object: BytecodeBlock::compile_expression(object, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
            asynchronous,
        });
        Ok(())
    }

    pub(super) fn compile_switch(
        &mut self,
        discriminant: &Expression,
        cases: &[SwitchCase],
    ) -> Result<()> {
        let mut bytecode_cases = Vec::with_capacity(cases.len());
        for case in cases {
            bytecode_cases.push(BytecodeSwitchCase {
                test: case
                    .test
                    .as_ref()
                    .map(|test| BytecodeBlock::compile_expression(test, self.layout))
                    .transpose()?,
                body: BytecodeBlock::compile_statements(
                    &case.statements,
                    StatementValue::Store,
                    self.layout,
                )?,
                body_updates_value: switch_statements_update_value(&case.statements),
            });
        }
        let scoped = switch_needs_lexical_scope(cases);
        let scope_statements = cases
            .iter()
            .flat_map(|case| case.statements.iter().cloned())
            .collect::<Vec<_>>();
        let scope_init = if scoped {
            Some(BytecodeBlock::compile_block_function_init(
                &scope_statements,
                self.layout,
            )?)
        } else {
            None
        };
        self.emit(BytecodeInstruction::Switch {
            discriminant: BytecodeBlock::compile_expression(discriminant, self.layout)?,
            cases: std::rc::Rc::from(bytecode_cases.into_boxed_slice()),
            scoped,
            scope_init,
        });
        Ok(())
    }

    pub(super) fn compile_try(
        &mut self,
        body: &[Statement],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Statement]>,
    ) -> Result<()> {
        let body_scoped = statements_need_lexical_scope(body);
        let body_block = if body_scoped {
            BytecodeBlock::compile_lexical_statements(body, StatementValue::Store, self.layout)?
        } else {
            BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout)?
        };
        let body_direct_throw = if body_scoped {
            None
        } else {
            BytecodeDirectThrow::from_unscoped_block_start(&body_block)
        };
        let finally_scoped = finally_body.is_some_and(statements_need_lexical_scope);
        let catch = catch
            .map(|catch| {
                let param = catch
                    .param
                    .as_ref()
                    .map(|param| self.compile_pattern(param).map(Rc::new))
                    .transpose()?;
                let mut param_bindings = Vec::new();
                if let Some(pattern) = &catch.param {
                    pattern.for_each_binding(&mut |binding| {
                        param_bindings.push(self.compile_binding(binding)?);
                        Ok(())
                    })?;
                }
                let body_scoped = statements_need_lexical_scope(&catch.body);
                let body = if body_scoped {
                    BytecodeBlock::compile_lexical_statements(
                        &catch.body,
                        StatementValue::Store,
                        self.layout,
                    )?
                } else {
                    BytecodeBlock::compile_statements(
                        &catch.body,
                        StatementValue::Store,
                        self.layout,
                    )?
                };
                Ok(BytecodeCatch {
                    param,
                    param_bindings: Rc::from(param_bindings.into_boxed_slice()),
                    body,
                    body_scoped,
                })
            })
            .transpose()?;
        let finally_body = finally_body
            .map(|body| {
                if statements_need_lexical_scope(body) {
                    BytecodeBlock::compile_lexical_statements(
                        body,
                        StatementValue::Store,
                        self.layout,
                    )
                } else {
                    BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout)
                }
            })
            .transpose()?;
        self.emit(BytecodeInstruction::Try {
            body: body_block,
            body_scoped,
            body_direct_throw,
            catch,
            finally_body,
            finally_scoped,
        });
        Ok(())
    }

    fn compile_statement_block(
        &self,
        statement: &Statement,
        value: StatementValue,
    ) -> Result<BytecodeBlock> {
        let mut compiler = Self::new(self.layout, statement.span());
        compiler.compile_statement(statement, value)?;
        compiler.finish()
    }

    fn emit_discard_branch_last(&mut self, value: StatementValue) {
        if value == StatementValue::Discard {
            self.emit_undefined_last();
        }
    }

    fn emit_undefined_last(&mut self) {
        self.emit(BytecodeInstruction::PushUndefined);
        self.emit(BytecodeInstruction::StoreLast);
    }

    fn compile_for_in_target(&self, target: &ForInTarget) -> Result<BytecodeForInTarget> {
        match target {
            ForInTarget::Binding { name, kind } => Ok(BytecodeForInTarget::Binding {
                name: self.compile_binding(name)?,
                kind: *kind,
            }),
            ForInTarget::PatternBinding { pattern, kind } => {
                Ok(BytecodeForInTarget::PatternBinding {
                    pattern: Rc::new(self.compile_pattern(pattern)?),
                    kind: *kind,
                })
            }
            ForInTarget::PatternAssignment { pattern, strict } => {
                Ok(BytecodeForInTarget::PatternAssignment(Rc::new(
                    self.compile_assignment_pattern(pattern, *strict)?,
                )))
            }
            ForInTarget::Assignment { target, strict } => self
                .compile_assignment_target_with_strict(target, *strict)
                .map(BytecodeForInTarget::Assignment),
        }
    }

    pub(super) fn compile_assignment_target_with_strict(
        &self,
        expr: &Expression,
        strict: bool,
    ) -> Result<BytecodeAssignmentTarget> {
        match expr.kind() {
            Expr::Identifier(name) => Ok(BytecodeAssignmentTarget::Binding(
                BytecodeBinding::compile_write(name, self.layout, strict)?,
            )),
            Expr::Call { .. } if !strict => Ok(BytecodeAssignmentTarget::WebCompatCall(
                BytecodeBlock::compile_expression(expr, self.layout)?,
            )),
            Expr::Member {
                object,
                property,
                access,
            } => {
                let property = Self::compile_property(property, *access);
                if let Some(index) = Self::compile_array_index(&property) {
                    return Ok(BytecodeAssignmentTarget::ArrayIndexProperty {
                        object: BytecodeBlock::compile_expression(object, self.layout)?,
                        property,
                        index,
                        strict,
                    });
                }
                Ok(BytecodeAssignmentTarget::StaticProperty {
                    object: BytecodeBlock::compile_expression(object, self.layout)?,
                    property,
                    strict,
                })
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => Ok(BytecodeAssignmentTarget::ComputedProperty {
                object: BytecodeBlock::compile_expression(object, self.layout)?,
                property: BytecodeBlock::compile_expression(property, self.layout)?,
                operand: Self::compile_dynamic_property(*access),
                strict,
            }),
            Expr::PrivateMember { object, name } => Ok(BytecodeAssignmentTarget::PrivateProperty {
                object: BytecodeBlock::compile_expression(object, self.layout)?,
                property: crate::bytecode::BytecodePrivateName::new(name.clone()),
            }),
            Expr::Parenthesized(expr) => self.compile_assignment_target_with_strict(expr, strict),
            _ => Err(Error::runtime("invalid bytecode assignment target")),
        }
    }
}

fn collect_label_chain<'a>(
    first_label: &StaticName,
    first_body: &'a Statement,
) -> (Rc<[StaticName]>, &'a Statement) {
    let mut labels = vec![first_label.clone()];
    let mut body = first_body;
    while let Stmt::Label {
        label,
        body: nested_body,
    } = body.kind()
    {
        labels.push(label.clone());
        body = nested_body;
    }
    (Rc::from(labels.into_boxed_slice()), body)
}

fn switch_needs_lexical_scope(cases: &[SwitchCase]) -> bool {
    cases
        .iter()
        .any(|case| statements_need_lexical_scope(&case.statements))
}

fn switch_statements_update_value(statements: &[Statement]) -> bool {
    statements.iter().any(|statement| match statement.kind() {
        Stmt::Expr(_) | Stmt::Switch { .. } | Stmt::Try { .. } => true,
        Stmt::DeclList(statements) | Stmt::Block(statements) => {
            switch_statements_update_value(statements)
        }
        Stmt::If {
            consequent,
            alternate,
            ..
        } => {
            switch_statement_updates_value(consequent)
                || alternate
                    .as_deref()
                    .is_some_and(switch_statement_updates_value)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::With { body, .. }
        | Stmt::Label { body, .. }
        | Stmt::For { body, .. }
        | Stmt::ForIn { body, .. }
        | Stmt::ForOf { body, .. } => switch_statement_updates_value(body),
        Stmt::Empty
        | Stmt::Debugger
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Throw(_)
        | Stmt::Return(_)
        | Stmt::FunctionDecl { .. }
        | Stmt::ImportBinding { .. }
        | Stmt::VarDecl { .. }
        | Stmt::PatternDecl { .. }
        | Stmt::ClassDecl { .. } => false,
    })
}

fn switch_statement_updates_value(statement: &Statement) -> bool {
    switch_statements_update_value(std::slice::from_ref(statement))
}
