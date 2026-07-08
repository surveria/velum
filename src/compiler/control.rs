use std::rc::Rc;

use crate::{
    ast::{CatchClause, DeclKind, Expr, ForInTarget, Stmt, SwitchCase},
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBlock, BytecodeCatch, BytecodeForInTarget,
        BytecodeInstruction, BytecodeSwitchCase,
    },
    error::{Error, Result},
    syntax::StaticName,
};

use super::{BytecodeCompiler, StatementValue};

impl BytecodeCompiler<'_> {
    pub(super) fn compile_if(
        &mut self,
        condition: &Expr,
        consequent: &Stmt,
        alternate: Option<&Stmt>,
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

    pub(super) fn compile_while(&mut self, condition: &Expr, body: &Stmt) -> Result<()> {
        self.compile_labeled_while(None, condition, body)
    }

    fn compile_labeled_while(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        condition: &Expr,
        body: &Stmt,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::While {
            labels,
            condition: BytecodeBlock::compile_expression(condition, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    pub(super) fn compile_do_while(&mut self, body: &Stmt, condition: &Expr) -> Result<()> {
        self.compile_labeled_do_while(None, body, condition)
    }

    fn compile_labeled_do_while(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        body: &Stmt,
        condition: &Expr,
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
        body: &Stmt,
        value: StatementValue,
    ) -> Result<()> {
        let (labels, labeled_body) = collect_label_chain(label, body);
        let labels = Some(labels);
        match labeled_body {
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
            } => {
                return self.compile_labeled_for_of(labels, target, object, body);
            }
            Stmt::Block(_)
            | Stmt::DeclList(_)
            | Stmt::Empty
            | Stmt::If { .. }
            | Stmt::Label { .. }
            | Stmt::Switch { .. }
            | Stmt::Try { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Throw(_)
            | Stmt::Return(_)
            | Stmt::FunctionDecl { .. }
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
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
    ) -> Result<()> {
        self.compile_labeled_for(None, init, condition, update, body)
    }

    fn compile_labeled_for(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        init: Option<&Stmt>,
        condition: Option<&Expr>,
        update: Option<&Expr>,
        body: &Stmt,
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
            scoped: for_init_needs_lexical_scope(init),
        });
        Ok(())
    }

    pub(super) fn compile_for_in(
        &mut self,
        target: &ForInTarget,
        object: &Expr,
        body: &Stmt,
    ) -> Result<()> {
        self.compile_labeled_for_in(None, target, object, body)
    }

    fn compile_labeled_for_in(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        target: &ForInTarget,
        object: &Expr,
        body: &Stmt,
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
        object: &Expr,
        body: &Stmt,
    ) -> Result<()> {
        self.compile_labeled_for_of(None, target, object, body)
    }

    fn compile_labeled_for_of(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        target: &ForInTarget,
        object: &Expr,
        body: &Stmt,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::ForOf {
            labels,
            target: self.compile_for_in_target(target)?,
            object: BytecodeBlock::compile_expression(object, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
        });
        Ok(())
    }

    pub(super) fn compile_switch(
        &mut self,
        discriminant: &Expr,
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
            });
        }
        self.emit(BytecodeInstruction::Switch {
            discriminant: BytecodeBlock::compile_expression(discriminant, self.layout)?,
            cases: std::rc::Rc::from(bytecode_cases.into_boxed_slice()),
        });
        Ok(())
    }

    pub(super) fn compile_try(
        &mut self,
        body: &[Stmt],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Stmt]>,
    ) -> Result<()> {
        self.emit(BytecodeInstruction::Try {
            body: BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout)?,
            catch: catch
                .map(|catch| {
                    Ok(BytecodeCatch {
                        param: catch
                            .param
                            .as_ref()
                            .map(|param| self.compile_binding(param))
                            .transpose()?,
                        body: BytecodeBlock::compile_statements(
                            &catch.body,
                            StatementValue::Store,
                            self.layout,
                        )?,
                    })
                })
                .transpose()?,
            finally_body: finally_body
                .map(|body| {
                    BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout)
                })
                .transpose()?,
        });
        Ok(())
    }

    fn compile_statement_block(
        &self,
        statement: &Stmt,
        value: StatementValue,
    ) -> Result<BytecodeBlock> {
        let mut compiler = Self::new(self.layout);
        compiler.compile_statement(statement, value)?;
        Ok(BytecodeBlock::from_instructions(compiler.instructions))
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
            ForInTarget::Assignment(expr) => self
                .compile_assignment_target(expr)
                .map(BytecodeForInTarget::Assignment),
        }
    }

    pub(super) fn compile_assignment_target(
        &self,
        expr: &Expr,
    ) -> Result<BytecodeAssignmentTarget> {
        match expr {
            Expr::Identifier(name) => Ok(BytecodeAssignmentTarget::Binding(
                self.compile_binding(name)?,
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
                    });
                }
                Ok(BytecodeAssignmentTarget::StaticProperty {
                    object: BytecodeBlock::compile_expression(object, self.layout)?,
                    property,
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
            }),
            Expr::Parenthesized(expr) => self.compile_assignment_target(expr),
            _ => Err(Error::runtime("invalid bytecode assignment target")),
        }
    }
}

fn collect_label_chain<'a>(
    first_label: &StaticName,
    first_body: &'a Stmt,
) -> (Rc<[StaticName]>, &'a Stmt) {
    let mut labels = vec![first_label.clone()];
    let mut body = first_body;
    while let Stmt::Label {
        label,
        body: nested_body,
    } = body
    {
        labels.push(label.clone());
        body = nested_body;
    }
    (Rc::from(labels.into_boxed_slice()), body)
}

fn for_init_needs_lexical_scope(init: Option<&Stmt>) -> bool {
    match init {
        Some(Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        }) => true,
        Some(Stmt::DeclList(statements)) => statements.iter().any(|statement| {
            matches!(
                statement,
                Stmt::VarDecl {
                    kind: DeclKind::Let | DeclKind::Const,
                    ..
                }
            )
        }),
        Some(_) | None => false,
    }
}
