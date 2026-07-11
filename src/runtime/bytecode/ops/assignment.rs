use crate::{
    bytecode::{
        BytecodeArrayIndex, BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock,
        BytecodeNumericBinaryOp, BytecodeProperty,
    },
    error::{Error, Result},
    runtime::binding::scope::BindingCell,
    runtime::numeric::{
        bitwise_and, bitwise_or, bitwise_xor, numeric_binary, shift_left, shift_right,
        shift_right_unsigned,
    },
    runtime::private::PrivateNameId,
    runtime::property::DynamicPropertyKey,
    runtime::{Context, abstract_operations::to_boolean, control::reference_error_undefined},
    syntax::{BinaryOp, StaticName, StaticPropertyAccessId, UpdateOp},
    value::Value,
};

#[derive(Debug, Clone)]
pub(in crate::runtime::bytecode) enum BytecodeAssignmentReference {
    Binding {
        name: BytecodeBinding,
        cell: Option<BindingCell>,
    },
    StaticProperty {
        object: Value,
        property: BytecodeProperty,
    },
    ArrayIndexProperty {
        object: Value,
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
    },
    ComputedProperty {
        object: Value,
        property_value: Value,
        property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
    },
    PrivateProperty {
        object: Value,
        name: PrivateNameId,
    },
}

impl BytecodeAssignmentReference {
    fn get(&self, context: &mut Context) -> Result<Value> {
        match self {
            Self::Binding { name, cell } => {
                let Some(cell) = cell else {
                    return context
                        .unresolved_global_property_value(name.name().name())?
                        .ok_or_else(|| reference_error_undefined(name.name()));
                };
                cell.value(name.name())
            }
            Self::StaticProperty { object, property } => {
                context.get_static_property_value(object, property.name(), property.access())
            }
            Self::ArrayIndexProperty {
                object,
                property,
                index,
            } => context.eval_bytecode_array_index_member(object, property, *index),
            Self::ComputedProperty {
                object,
                property_value,
                property,
                access,
            } => {
                if let Some(value) =
                    context.eval_dynamic_array_index_member(object, property_value)?
                {
                    return Ok(value);
                }
                context.get_cached_dynamic_property_value(object, property, *access)
            }
            Self::PrivateProperty { object, name } => context.read_private_slot(object, name),
        }
    }

    pub(in crate::runtime::bytecode) fn set(
        &self,
        context: &mut Context,
        value: Value,
    ) -> Result<()> {
        match self {
            Self::Binding { name, cell } => {
                if let Some(cell) = cell {
                    return context.assign_bytecode_cell(name, cell, value);
                }
                context.assign_bytecode_or_create_sloppy_global(name, value)
            }
            Self::StaticProperty { object, property } => {
                context.set_static_property_value(object, property.name(), property.access(), value)
            }
            Self::ArrayIndexProperty {
                object,
                property,
                index,
            } => context.set_bytecode_array_index_property(object, property, *index, value),
            Self::ComputedProperty {
                object,
                property_value,
                property,
                access,
            } => {
                if context.set_dynamic_array_index_property(
                    object,
                    property_value,
                    value.clone(),
                )? {
                    return Ok(());
                }
                let mut property = property.clone();
                context.set_cached_dynamic_property_value(object, &mut property, *access, value)
            }
            Self::PrivateProperty { object, name } => {
                context.write_private_slot(object, name, value)
            }
        }
    }

    pub(in crate::runtime::bytecode) fn root_values(&self) -> Vec<&Value> {
        match self {
            Self::Binding { .. } => Vec::new(),
            Self::StaticProperty { object, .. }
            | Self::ArrayIndexProperty { object, .. }
            | Self::PrivateProperty { object, .. } => {
                vec![object]
            }
            Self::ComputedProperty {
                object,
                property_value,
                ..
            } => vec![object, property_value],
        }
    }
}

impl Context {
    pub(in crate::runtime::bytecode) fn eval_bytecode_update_binding(
        &mut self,
        name: &BytecodeBinding,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let binding = self
            .get_binding_bytecode(name)?
            .ok_or_else(|| reference_error_undefined(name.name()))?;
        let old_value = binding.value(name.name())?;
        let new_value = Self::updated_bytecode_number(&old_value, op)?;
        self.checked_value(new_value.clone())?;
        self.assign_bytecode_cell(name, &binding, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_update_static_property(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        if let Some((old_value, new_value)) = self.try_cached_static_property_read_modify_write(
            object,
            property,
            access,
            |_, value| Self::updated_bytecode_number(value, op),
        )? {
            return Ok(if prefix { new_value } else { old_value });
        }
        let old_value = self.get_static_property_value(object, property, access)?;
        let new_value = Self::updated_bytecode_number(&old_value, op)?;
        self.set_static_property_value(object, property, access, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_update_dynamic_property(
        &mut self,
        object: &Value,
        mut property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        if let Some((old_value, new_value)) = self.try_cached_dynamic_property_read_modify_write(
            object,
            &mut property,
            access,
            |_, value| Self::updated_bytecode_number(value, op),
        )? {
            return Ok(if prefix { new_value } else { old_value });
        }
        let old_value = self.get_cached_dynamic_property_value(object, &property, access)?;
        let new_value = Self::updated_bytecode_number(&old_value, op)?;
        self.set_cached_dynamic_property_value(object, &mut property, access, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn updated_bytecode_number(
        value: &Value,
        op: UpdateOp,
    ) -> Result<Value> {
        let Some(number) = value.as_number() else {
            return Err(Error::runtime("update operator expects a number"));
        };
        let updated = match op {
            UpdateOp::Increment => number + 1.0,
            UpdateOp::Decrement => number - 1.0,
        };
        Ok(Value::Number(updated))
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_binding_compound_assignment(
        &mut self,
        op: BinaryOp,
        name: &BytecodeBinding,
        right: &Value,
    ) -> Result<Value> {
        let binding = self
            .get_or_materialize_binding_bytecode(name)?
            .ok_or_else(|| reference_error_undefined(name.name()))?;
        let old_value = binding.value(name.name())?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        self.assign_bytecode_cell(name, &binding, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_static_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        right: &Value,
    ) -> Result<Value> {
        if let Some((_, value)) = self.try_cached_static_property_read_modify_write(
            object,
            property,
            access,
            |context, old_value| context.eval_bytecode_compound_value(op, old_value, right),
        )? {
            return Ok(value);
        }
        let old_value = self.get_static_property_value(object, property, access)?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        self.set_static_property_value(object, property, access, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_dynamic_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        mut property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
        right: &Value,
    ) -> Result<Value> {
        if let Some((_, value)) = self.try_cached_dynamic_property_read_modify_write(
            object,
            &mut property,
            access,
            |context, old_value| context.eval_bytecode_compound_value(op, old_value, right),
        )? {
            return Ok(value);
        }
        let old_value = self.get_cached_dynamic_property_value(object, &property, access)?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        self.set_cached_dynamic_property_value(object, &mut property, access, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_compound_value(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let (Value::Number(left), Value::Number(right)) = (left, right)
            && let Some(op) = BytecodeNumericBinaryOp::from_binary(op)
        {
            return self.eval_bytecode_number_binary(
                op,
                &Value::Number(*left),
                &Value::Number(*right),
            );
        }
        let value = match op {
            BinaryOp::Add => self.add(left, right)?,
            BinaryOp::Sub => numeric_binary(self, left, right, "-=", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(self, left, right, "*=", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(self, left, right, "/=", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(self, left, right, "%=", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(self, left, right, "**=", f64::powf)?,
            BinaryOp::BitAnd => bitwise_and(self, left, right)?,
            BinaryOp::BitOr => bitwise_or(self, left, right)?,
            BinaryOp::BitXor => bitwise_xor(self, left, right)?,
            BinaryOp::ShiftLeft => shift_left(self, left, right)?,
            BinaryOp::ShiftRight => shift_right(self, left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(self, left, right)?,
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::InstanceOf
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::NullishCoalescing => {
                return Err(Error::runtime("invalid compound assignment operator"));
            }
        };
        self.runtime_value(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_logical_assignment(
        &mut self,
        op: BinaryOp,
        target: &BytecodeAssignmentTarget,
        value: &BytecodeBlock,
    ) -> Result<Value> {
        let reference = self.eval_bytecode_assignment_reference(target)?;
        let current = reference.get(self)?;
        if !logical_assignment_should_store(op, &current)? {
            return self.runtime_value(current);
        }
        let value = self.eval_bytecode_expression(value)?;
        reference.set(self, value.clone())?;
        self.runtime_value(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_assignment_reference(
        &mut self,
        target: &BytecodeAssignmentTarget,
    ) -> Result<BytecodeAssignmentReference> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => {
                let cell = self.get_or_materialize_binding_bytecode(name)?;
                Ok(BytecodeAssignmentReference::Binding {
                    name: name.clone(),
                    cell,
                })
            }
            BytecodeAssignmentTarget::StaticProperty { object, property } => {
                Ok(BytecodeAssignmentReference::StaticProperty {
                    object: self.eval_bytecode_expression(object)?,
                    property: property.clone(),
                })
            }
            BytecodeAssignmentTarget::ArrayIndexProperty {
                object,
                property,
                index,
            } => Ok(BytecodeAssignmentReference::ArrayIndexProperty {
                object: self.eval_bytecode_expression(object)?,
                property: property.clone(),
                index: *index,
            }),
            BytecodeAssignmentTarget::ComputedProperty {
                object,
                property,
                operand,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                let property_value = self.eval_bytecode_expression(property)?;
                let property = self.dynamic_property_key(&property_value)?;
                Ok(BytecodeAssignmentReference::ComputedProperty {
                    object,
                    property_value,
                    property,
                    access: operand.access(),
                })
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                let name = self.resolve_private_name(property)?;
                Ok(BytecodeAssignmentReference::PrivateProperty { object, name })
            }
        }
    }

    pub(in crate::runtime::bytecode) fn assign_bytecode_target(
        &mut self,
        target: &BytecodeAssignmentTarget,
        value: Value,
    ) -> Result<()> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => {
                self.assign_bytecode_or_create_sloppy_global(name, value)
            }
            BytecodeAssignmentTarget::StaticProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                self.set_static_property_value(&object, property.name(), property.access(), value)
            }
            BytecodeAssignmentTarget::ArrayIndexProperty {
                object,
                property,
                index,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                self.set_bytecode_array_index_property(&object, property, *index, value)
            }
            BytecodeAssignmentTarget::ComputedProperty {
                object,
                property,
                operand,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                let property = self.eval_bytecode_expression(property)?;
                let mut property = self.dynamic_property_key(&property)?;
                self.set_cached_dynamic_property_value(
                    &object,
                    &mut property,
                    operand.access(),
                    value,
                )
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                let name = self.resolve_private_name(property)?;
                self.write_private_slot(&object, &name, value)
            }
        }
    }
}

fn logical_assignment_should_store(op: BinaryOp, value: &Value) -> Result<bool> {
    match op {
        BinaryOp::LogicalAnd => Ok(to_boolean(value)),
        BinaryOp::LogicalOr => Ok(!to_boolean(value)),
        BinaryOp::NullishCoalescing => Ok(matches!(value, Value::Undefined | Value::Null)),
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Pow
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::StrictEqual
        | BinaryOp::StrictNotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::In
        | BinaryOp::InstanceOf
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::ShiftRightUnsigned => {
            Err(Error::runtime("invalid logical assignment operator"))
        }
    }
}
