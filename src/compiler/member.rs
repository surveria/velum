use super::{
    ARRAY_LENGTH_PROPERTY, BinaryOp, BytecodeBinding, BytecodeBlock, BytecodeCompiler,
    BytecodeInstruction, Error, Expr, Expression, Result, StaticName, StaticPropertyAccessId,
    UpdateOp,
};
use crate::bytecode::BytecodePrivateName;

impl BytecodeCompiler<'_> {
    pub(super) fn compile_private_expression(&mut self, expr: &Expr) -> Result<()> {
        match expr {
            Expr::PrivateMember { object, name } => {
                self.compile_private_member_expr(object, name)?;
            }
            Expr::PrivateAssignment { object, name, expr } => {
                self.compile_private_assignment(object, name, expr)?;
            }
            Expr::PrivateIn { name, object } => self.compile_private_in_expr(name, object)?,
            _ => return Err(Error::runtime("expression is not a private operation")),
        }
        Ok(())
    }

    pub(super) fn compile_static_property_assignment(
        &mut self,
        object: &Expression,
        property: &StaticName,
        access: StaticPropertyAccessId,
        expr: &Expression,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(expr)?;
        let property = Self::compile_property(property, access);
        if let Some(index) = Self::compile_array_index(&property) {
            self.emit(BytecodeInstruction::ArrayIndexAssign { property, index });
        } else {
            self.emit(BytecodeInstruction::StaticPropertyAssign { property });
        }
        Ok(())
    }

    pub(super) fn compile_computed_property_assignment(
        &mut self,
        object: &Expression,
        property: &Expression,
        access: StaticPropertyAccessId,
        expr: &Expression,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(property)?;
        self.compile_expr(expr)?;
        self.emit(BytecodeInstruction::ComputedPropertyAssign {
            property: Self::compile_dynamic_property(access),
        });
        Ok(())
    }

    pub(super) fn compile_static_member_expr(
        &mut self,
        object: &Expression,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<()> {
        self.compile_expr(object)?;
        let property = Self::compile_property(property, access);
        if property.name().as_str() == ARRAY_LENGTH_PROPERTY {
            self.emit(BytecodeInstruction::ArrayLength { property });
        } else if let Some(index) = Self::compile_array_index(&property) {
            self.emit(BytecodeInstruction::ArrayIndexMember { property, index });
        } else {
            self.emit(BytecodeInstruction::StaticMember { property });
        }
        Ok(())
    }

    pub(super) fn compile_computed_member_expr(
        &mut self,
        object: &Expression,
        property: &Expression,
        access: StaticPropertyAccessId,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(property)?;
        self.emit(BytecodeInstruction::ComputedMember {
            property: Self::compile_dynamic_property(access),
        });
        Ok(())
    }

    pub(super) fn compile_delete_expr(&mut self, expr: &Expression) -> Result<()> {
        match expr.kind() {
            Expr::Parenthesized(expr) => self.compile_delete_expr(expr),
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::DeleteBinding(
                    self.compile_binding(name)?,
                ));
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::DeleteStaticProperty {
                    property: Self::compile_property(property, *access),
                });
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::DeleteComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                });
                Ok(())
            }
            Expr::PrivateMember { .. } => Err(Error::runtime("private members cannot be deleted")),
            _ => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::DeleteValue);
                Ok(())
            }
        }
    }

    pub(super) fn compile_update_expr(
        &mut self,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
        expr: &Expression,
    ) -> Result<()> {
        match expr.kind() {
            Expr::Identifier(name) => {
                self.emit(BytecodeInstruction::UpdateBinding {
                    name: BytecodeBinding::compile_write(name, self.layout, strict)?,
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                let property = Self::compile_property(property, *access);
                if let Some(index) = Self::compile_array_index(&property) {
                    self.emit(BytecodeInstruction::UpdateArrayIndexProperty {
                        property,
                        index,
                        op,
                        prefix,
                    });
                } else {
                    self.emit(BytecodeInstruction::UpdateStaticProperty {
                        property,
                        op,
                        prefix,
                    });
                }
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.emit(BytecodeInstruction::UpdateComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::PrivateMember { object, name } => {
                self.compile_expr(object)?;
                self.emit(BytecodeInstruction::UpdatePrivateProperty {
                    property: BytecodePrivateName::new(name.clone()),
                    op,
                    prefix,
                });
                Ok(())
            }
            Expr::Parenthesized(expr) => self.compile_update_expr(op, prefix, strict, expr),
            _ => Err(Error::runtime("invalid bytecode update target")),
        }
    }

    pub(super) fn compile_compound_assignment(
        &mut self,
        op: BinaryOp,
        strict: bool,
        target: &Expression,
        expr: &Expression,
    ) -> Result<()> {
        if matches!(
            op,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
        ) {
            let target = self.compile_assignment_target_with_strict(target, strict)?;
            self.emit(BytecodeInstruction::LogicalAssignment {
                op,
                target,
                value: BytecodeBlock::compile_expression(expr, self.layout)?,
            });
            return Ok(());
        }
        match target.kind() {
            Expr::Identifier(name) => {
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundStoreBinding {
                    name: BytecodeBinding::compile_write(name, self.layout, strict)?,
                    op,
                });
                Ok(())
            }
            Expr::Member {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(expr)?;
                let property = Self::compile_property(property, *access);
                if let Some(index) = Self::compile_array_index(&property) {
                    self.emit(BytecodeInstruction::CompoundArrayIndexProperty {
                        property,
                        index,
                        op,
                    });
                } else {
                    self.emit(BytecodeInstruction::CompoundStaticProperty { property, op });
                }
                Ok(())
            }
            Expr::ComputedMember {
                object,
                property,
                access,
            } => {
                self.compile_expr(object)?;
                self.compile_expr(property)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundComputedProperty {
                    property: Self::compile_dynamic_property(*access),
                    op,
                });
                Ok(())
            }
            Expr::PrivateMember { object, name } => {
                self.compile_expr(object)?;
                self.compile_expr(expr)?;
                self.emit(BytecodeInstruction::CompoundPrivateProperty {
                    property: BytecodePrivateName::new(name.clone()),
                    op,
                });
                Ok(())
            }
            Expr::Parenthesized(target) => {
                self.compile_compound_assignment(op, strict, target, expr)
            }
            _ => Err(Error::runtime(
                "invalid bytecode compound assignment target",
            )),
        }
    }

    /// Compiles `obj.#name` reads: the object is pushed and the private
    /// instruction resolves the slot with brand-check semantics.
    pub(super) fn compile_private_member_expr(
        &mut self,
        object: &Expression,
        name: &StaticName,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.emit(BytecodeInstruction::PrivateMember {
            property: BytecodePrivateName::new(name.clone()),
        });
        Ok(())
    }

    /// Compiles `obj.#name = value` writes with the value left on the stack.
    pub(super) fn compile_private_assignment(
        &mut self,
        object: &Expression,
        name: &StaticName,
        expr: &Expression,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.compile_expr(expr)?;
        self.emit(BytecodeInstruction::PrivateAssign {
            property: BytecodePrivateName::new(name.clone()),
        });
        Ok(())
    }

    /// Compiles the `#name in obj` ergonomic brand check.
    pub(super) fn compile_private_in_expr(
        &mut self,
        name: &StaticName,
        object: &Expression,
    ) -> Result<()> {
        self.compile_expr(object)?;
        self.emit(BytecodeInstruction::PrivateIn {
            property: BytecodePrivateName::new(name.clone()),
        });
        Ok(())
    }
}
