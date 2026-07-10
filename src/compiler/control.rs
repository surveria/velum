use std::rc::Rc;

use crate::{
    ast::{CatchClause, DeclKind, Expr, Expression, ForInTarget, Statement, Stmt, SwitchCase},
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock, BytecodeCatch,
        BytecodeCatchFastPath, BytecodeCompletion, BytecodeDirectThrow, BytecodeForInTarget,
        BytecodeInstruction, BytecodeNumericBinaryOp, BytecodeNumericEqualityOp,
        BytecodeSwitchCase, BytecodeTryFinallyFastPath,
    },
    error::{Error, Result},
    syntax::StaticName,
    value::Value,
};

use super::{BytecodeCompiler, StatementValue, statements_need_lexical_scope};

impl BytecodeCompiler<'_> {
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
            scoped: for_init_needs_lexical_scope(init),
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
    ) -> Result<()> {
        self.compile_labeled_for_of(None, target, object, body)
    }

    fn compile_labeled_for_of(
        &mut self,
        labels: Option<Rc<[StaticName]>>,
        target: &ForInTarget,
        object: &Expression,
        body: &Statement,
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
            });
        }
        self.emit(BytecodeInstruction::Switch {
            discriminant: BytecodeBlock::compile_expression(discriminant, self.layout)?,
            cases: std::rc::Rc::from(bytecode_cases.into_boxed_slice()),
            scoped: switch_needs_lexical_scope(cases),
        });
        Ok(())
    }

    pub(super) fn compile_try(
        &mut self,
        body: &[Statement],
        catch: Option<&CatchClause>,
        finally_body: Option<&[Statement]>,
    ) -> Result<()> {
        let body_block =
            BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout)?;
        let body_scoped = statements_need_lexical_scope(body);
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
                    .map(|param| self.compile_binding(param))
                    .transpose()?;
                let body = BytecodeBlock::compile_statements(
                    &catch.body,
                    StatementValue::Store,
                    self.layout,
                )?;
                let body_scoped = statements_need_lexical_scope(&catch.body);
                let body_fast_path =
                    BytecodeCatchFastPath::from_unscoped_body(param.as_ref(), &body, body_scoped);
                Ok(BytecodeCatch {
                    param,
                    body,
                    body_scoped,
                    body_fast_path,
                })
            })
            .transpose()?;
        let finally_body = finally_body
            .map(|body| BytecodeBlock::compile_statements(body, StatementValue::Store, self.layout))
            .transpose()?;
        let try_fast_path = bytecode_try_finally_fast_path(
            &body_block,
            body_scoped,
            catch.as_ref(),
            finally_body.as_ref(),
            finally_scoped,
        )
        .map(Box::new);
        self.emit(BytecodeInstruction::Try {
            body: body_block,
            body_scoped,
            body_direct_throw,
            try_fast_path,
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
            ForInTarget::Assignment(expr) => self
                .compile_assignment_target(expr)
                .map(BytecodeForInTarget::Assignment),
        }
    }

    pub(super) fn compile_assignment_target(
        &self,
        expr: &Expression,
    ) -> Result<BytecodeAssignmentTarget> {
        match expr.kind() {
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

fn bytecode_try_finally_fast_path(
    body: &BytecodeBlock,
    body_scoped: bool,
    catch: Option<&BytecodeCatch>,
    finally_body: Option<&BytecodeBlock>,
    finally_scoped: bool,
) -> Option<BytecodeTryFinallyFastPath> {
    if body_scoped || finally_scoped {
        return None;
    }
    let [
        BytecodeInstruction::LoadBinding(index),
        BytecodeInstruction::PushLiteral(Value::Number(index_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        BytecodeInstruction::PushLiteral(Value::Number(throw_right)),
        BytecodeInstruction::NumberEquality(BytecodeNumericEqualityOp::StrictEqual),
        BytecodeInstruction::JumpIfFalse(alternate),
        BytecodeInstruction::PushLiteral(Value::Number(throw_value)),
        BytecodeInstruction::Complete(BytecodeCompletion::Throw),
        BytecodeInstruction::Jump(end),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadBinding(try_total_read),
        BytecodeInstruction::PushLiteral(Value::Number(try_add)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(try_total_write),
        BytecodeInstruction::StoreLast,
    ] = body.instructions()
    else {
        return None;
    };
    if alternate.index() != 9
        || end.index() != 11
        || !same_bytecode_binding(try_total_read, try_total_write)
    {
        return None;
    }
    let catch = catch?;
    if catch.body_scoped {
        return None;
    }
    let catch_param = catch.param.as_ref()?;
    let [
        BytecodeInstruction::LoadBinding(catch_total_read),
        BytecodeInstruction::LoadBinding(catch_error),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(catch_total_write),
        BytecodeInstruction::StoreLast,
    ] = catch.body.instructions()
    else {
        return None;
    };
    let [
        BytecodeInstruction::LoadBinding(finally_total_read),
        BytecodeInstruction::PushLiteral(Value::Number(finally_add)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(finally_total_write),
        BytecodeInstruction::StoreLast,
    ] = finally_body?.instructions()
    else {
        return None;
    };
    if !same_bytecode_binding(catch_param, catch_error)
        || !same_bytecode_binding(try_total_write, catch_total_read)
        || !same_bytecode_binding(try_total_write, catch_total_write)
        || !same_bytecode_binding(try_total_write, finally_total_read)
        || !same_bytecode_binding(try_total_write, finally_total_write)
    {
        return None;
    }
    Some(BytecodeTryFinallyFastPath {
        index: index.clone(),
        index_mask: *index_mask,
        throw_right: *throw_right,
        throw_value: *throw_value,
        total: try_total_write.clone(),
        try_add: *try_add,
        finally_add: *finally_add,
    })
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
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

fn for_init_needs_lexical_scope(init: Option<&Statement>) -> bool {
    let Some(init) = init else {
        return false;
    };
    match init.kind() {
        Stmt::VarDecl {
            kind: DeclKind::Let | DeclKind::Const,
            ..
        } => true,
        Stmt::DeclList(statements) => statements.iter().any(|statement| {
            matches!(
                statement.kind(),
                Stmt::VarDecl {
                    kind: DeclKind::Let | DeclKind::Const,
                    ..
                }
            )
        }),
        _ => false,
    }
}

fn switch_needs_lexical_scope(cases: &[SwitchCase]) -> bool {
    cases
        .iter()
        .any(|case| statements_need_lexical_scope(&case.statements))
}
