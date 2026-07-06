use crate::{
    ast::{CatchClause, DeclKind, Expr, ForInTarget, Stmt, SwitchCase},
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBlock, BytecodeCatch, BytecodeForInTarget,
        BytecodeInstruction, BytecodeSwitchCase,
    },
    error::{Error, Result},
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
        let condition = BytecodeBlock::compile_expression(condition, self.layout)?;
        let consequent = self.compile_statement_block(consequent, value)?;
        let alternate = alternate
            .map(|alternate| self.compile_statement_block(alternate, value))
            .transpose()?;
        self.emit(BytecodeInstruction::If {
            condition,
            consequent,
            alternate,
        });
        Ok(())
    }

    pub(super) fn compile_while(&mut self, condition: &Expr, body: &Stmt) -> Result<()> {
        self.emit(BytecodeInstruction::While {
            condition: BytecodeBlock::compile_expression(condition, self.layout)?,
            body: self.compile_statement_block(body, StatementValue::Store)?,
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
        self.emit(BytecodeInstruction::For {
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
        self.emit(BytecodeInstruction::ForIn {
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

    fn compile_for_in_target(&self, target: &ForInTarget) -> Result<BytecodeForInTarget> {
        match target {
            ForInTarget::Binding { name, kind } => Ok(BytecodeForInTarget::Binding {
                name: self.compile_binding(name)?,
                kind: *kind,
            }),
            ForInTarget::Assignment(expr) => self
                .compile_assignment_target(expr)
                .map(BytecodeForInTarget::Assignment),
        }
    }

    fn compile_assignment_target(&self, expr: &Expr) -> Result<BytecodeAssignmentTarget> {
        match expr {
            Expr::Identifier(name) => Ok(BytecodeAssignmentTarget::Binding(
                self.compile_binding(name)?,
            )),
            Expr::Member {
                object,
                property,
                access,
            } => Ok(BytecodeAssignmentTarget::StaticProperty {
                object: BytecodeBlock::compile_expression(object, self.layout)?,
                property: Self::compile_property(property, *access),
            }),
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
