#[cfg(not(feature = "std"))]
use crate::prelude::*;

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
    runtime::{
        Context,
        abstract_operations::{NumericValue, SetFailureBehavior, to_boolean},
        control::reference_error_undefined,
        object::PropertyLookup,
    },
    syntax::{BinaryOp, StaticName, StaticPropertyAccessId, UpdateOp},
    value::{ErrorName, Value},
};

const CALL_ASSIGNMENT_ERROR: &str = "function call is not a valid assignment target";

pub(in crate::runtime::bytecode) fn web_compat_call_assignment_error() -> Error {
    Error::exception(ErrorName::ReferenceError, CALL_ASSIGNMENT_ERROR)
}

#[derive(Debug, Clone)]
pub(in crate::runtime::bytecode) enum BytecodeAssignmentReference {
    Binding {
        name: BytecodeBinding,
        cell: Option<BindingCell>,
    },
    WithBinding {
        name: BytecodeBinding,
        reference: crate::runtime::binding::WithBindingReference,
    },
    StaticProperty {
        object: Value,
        property: BytecodeProperty,
        strict: bool,
    },
    ArrayIndexProperty {
        object: Value,
        property: BytecodeProperty,
        index: BytecodeArrayIndex,
        strict: bool,
    },
    ComputedProperty {
        object: Value,
        property_value: Value,
        property: Option<DynamicPropertyKey>,
        access: StaticPropertyAccessId,
        strict: bool,
    },
    PrivateProperty {
        object: Value,
        name: PrivateNameId,
    },
    SuperProperty {
        base: Value,
        receiver: Value,
        property: DynamicPropertyKey,
        strict: bool,
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
            Self::WithBinding { name, reference } => reference.get(context, name),
            Self::StaticProperty {
                object, property, ..
            } => context.get_static_property_value(object, property.name(), property.access()),
            Self::ArrayIndexProperty {
                object,
                property,
                index,
                ..
            } => context.eval_bytecode_array_index_member(object, property, *index),
            Self::ComputedProperty {
                object,
                property_value,
                property,
                access,
                ..
            } => {
                if let Some(value) =
                    context.eval_dynamic_array_index_member(object, property_value)?
                {
                    return Ok(value);
                }
                let property = match property {
                    Some(property) => property.clone(),
                    None => context.dynamic_property_key(property_value)?,
                };
                context.get_cached_dynamic_property_value(object, &property, *access)
            }
            Self::PrivateProperty { object, name } => context.read_private_slot(object, name),
            Self::SuperProperty {
                base,
                receiver,
                property,
                ..
            } => context.get_super_property(base, receiver, property.lookup()),
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
            Self::WithBinding { name, reference } => reference.set(context, name, value),
            Self::StaticProperty {
                object,
                property,
                strict,
            } => context.set_bytecode_static_property_reference(
                object,
                property.name(),
                property.access(),
                value,
                *strict,
            ),
            Self::ArrayIndexProperty {
                object,
                property,
                index,
                strict,
            } => {
                context.set_bytecode_array_index_property(object, property, *index, value, *strict)
            }
            Self::ComputedProperty {
                object,
                property_value,
                property,
                access,
                strict,
            } => {
                let mut property = match property {
                    Some(property) => property.clone(),
                    None => context.dynamic_property_key(property_value)?,
                };
                context.set_bytecode_dynamic_property_reference(
                    object,
                    &mut property,
                    *access,
                    value,
                    *strict,
                )
            }
            Self::PrivateProperty { object, name } => {
                context.write_private_slot(object, name, value)
            }
            Self::SuperProperty {
                base,
                receiver,
                property,
                strict,
            } => context.set_super_property_parts(base, receiver, property, value, *strict),
        }
    }

    pub(in crate::runtime::bytecode) fn root_values(&self) -> Vec<&Value> {
        match self {
            Self::Binding { .. } => Vec::new(),
            Self::WithBinding { reference, .. } => reference.object().into_iter().collect(),
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
            Self::SuperProperty { base, receiver, .. } => vec![base, receiver],
        }
    }
}

impl Context {
    pub(in crate::runtime::bytecode) fn set_bytecode_static_property_reference(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        value: Value,
        strict: bool,
    ) -> Result<()> {
        if !strict && self.bytecode_property_target_is_plain_array(object)? {
            return self.set_static_property_value(object, property, access, value);
        }
        if self.try_set_cached_static_own_property_value(object, property, access, value.clone())? {
            return Ok(());
        }
        let value = self.runtime_value(value)?;
        let key = self.intern_static_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.as_str(), key);
        self.set(object, lookup, value, object, bytecode_set_failure(strict))?;
        Ok(())
    }

    pub(in crate::runtime::bytecode) fn set_bytecode_dynamic_property_reference(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        access: StaticPropertyAccessId,
        value: Value,
        strict: bool,
    ) -> Result<()> {
        if !strict && self.bytecode_property_target_is_plain_array(object)? {
            return self.set_cached_dynamic_property_value(object, property, access, value);
        }
        if self.try_set_cached_dynamic_own_property_value(
            object,
            property,
            access,
            value.clone(),
        )? {
            return Ok(());
        }
        let value = self.runtime_value(value)?;
        self.set(
            object,
            property.lookup(),
            value,
            object,
            bytecode_set_failure(strict),
        )?;
        Ok(())
    }

    fn bytecode_property_target_is_plain_array(&self, object: &Value) -> Result<bool> {
        let Value::Object(id) = object else {
            return Ok(false);
        };
        Ok(self.objects.array_len_if_array(*id)?.is_some()
            && !self
                .objects
                .prototype_chain_requires_semantic_index_write(*id)?)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_update_binding(
        &mut self,
        name: &BytecodeBinding,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let reference = self.eval_bytecode_binding_assignment_reference(name)?;
        let old_value = reference.get(self)?;
        let (old_value, new_value) = self.bytecode_update_values(&old_value, op)?;
        self.checked_value(new_value.clone())?;
        reference.set(self, new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_update_static_property(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        if !strict
            && let Some((old_value, new_value)) = self
                .try_cached_static_property_read_modify_write(
                    object,
                    property,
                    access,
                    |context, value| context.bytecode_update_values(value, op),
                )?
        {
            return Ok(if prefix { new_value } else { old_value });
        }
        let old_value = self.get_static_property_value(object, property, access)?;
        let (old_value, new_value) = self.bytecode_update_values(&old_value, op)?;
        self.set_bytecode_static_property_reference(
            object,
            property,
            access,
            new_value.clone(),
            strict,
        )?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_update_dynamic_property(
        &mut self,
        object: &Value,
        mut property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
    ) -> Result<Value> {
        if !strict
            && let Some((old_value, new_value)) = self
                .try_cached_dynamic_property_read_modify_write(
                    object,
                    &mut property,
                    access,
                    |context, value| context.bytecode_update_values(value, op),
                )?
        {
            return Ok(if prefix { new_value } else { old_value });
        }
        let old_value = self.get_cached_dynamic_property_value(object, &property, access)?;
        let (old_value, new_value) = self.bytecode_update_values(&old_value, op)?;
        self.set_bytecode_dynamic_property_reference(
            object,
            &mut property,
            access,
            new_value.clone(),
            strict,
        )?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(in crate::runtime::bytecode) fn bytecode_update_values(
        &mut self,
        value: &Value,
        op: UpdateOp,
    ) -> Result<(Value, Value)> {
        match self.to_numeric(value)? {
            NumericValue::Number(number) => {
                let updated = match op {
                    UpdateOp::Increment => number + 1.0,
                    UpdateOp::Decrement => number - 1.0,
                };
                Ok((Value::Number(number), Value::Number(updated)))
            }
            NumericValue::BigInt(integer) => {
                let one = crate::value::JsBigInt::from_u64(1);
                let updated = match op {
                    UpdateOp::Increment => integer.add(&one),
                    UpdateOp::Decrement => integer.sub(&one),
                };
                Ok((Value::BigInt(integer), self.bigint_value(updated)?))
            }
        }
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_binding_compound_assignment(
        &mut self,
        op: BinaryOp,
        name: &BytecodeBinding,
        right: &Value,
    ) -> Result<Value> {
        let reference = self.eval_bytecode_binding_assignment_reference(name)?;
        let old_value = reference.get(self)?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        reference.set(self, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_static_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        right: &Value,
        strict: bool,
    ) -> Result<Value> {
        if !strict
            && let Some((_, value)) = self.try_cached_static_property_read_modify_write(
                object,
                property,
                access,
                |context, old_value| {
                    context
                        .eval_bytecode_compound_value(op, old_value, right)
                        .map(|new_value| (old_value.clone(), new_value))
                },
            )?
        {
            return Ok(value);
        }
        let old_value = self.get_static_property_value(object, property, access)?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        self.set_bytecode_static_property_reference(
            object,
            property,
            access,
            value.clone(),
            strict,
        )?;
        Ok(value)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_dynamic_compound_assignment(
        &mut self,
        op: BinaryOp,
        object: &Value,
        mut property: DynamicPropertyKey,
        access: StaticPropertyAccessId,
        right: &Value,
        strict: bool,
    ) -> Result<Value> {
        if !strict
            && let Some((_, value)) = self.try_cached_dynamic_property_read_modify_write(
                object,
                &mut property,
                access,
                |context, old_value| {
                    context
                        .eval_bytecode_compound_value(op, old_value, right)
                        .map(|new_value| (old_value.clone(), new_value))
                },
            )?
        {
            return Ok(value);
        }
        let old_value = self.get_cached_dynamic_property_value(object, &property, access)?;
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        self.set_bytecode_dynamic_property_reference(
            object,
            &mut property,
            access,
            value.clone(),
            strict,
        )?;
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
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem | BinaryOp::Pow => {
                numeric_binary(self, left, right, op)?
            }
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
        if matches!(
            op,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
        ) {
            if !logical_assignment_should_store(self, op, &current)? {
                return self.runtime_value(current);
            }
            let value = self.eval_bytecode_expression(value)?;
            reference.set(self, value.clone())?;
            return self.runtime_value(value);
        }
        let right = self.eval_bytecode_expression(value)?;
        let result = self.eval_bytecode_compound_value(op, &current, &right)?;
        reference.set(self, result.clone())?;
        self.runtime_value(result)
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_assignment_reference(
        &mut self,
        target: &BytecodeAssignmentTarget,
    ) -> Result<BytecodeAssignmentReference> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => {
                self.eval_bytecode_binding_assignment_reference(name)
            }
            BytecodeAssignmentTarget::WebCompatCall(target) => {
                self.eval_bytecode_expression(target)?;
                Err(web_compat_call_assignment_error())
            }
            BytecodeAssignmentTarget::StaticProperty {
                object,
                property,
                strict,
            } => Ok(BytecodeAssignmentReference::StaticProperty {
                object: self.eval_bytecode_expression(object)?,
                property: property.clone(),
                strict: *strict,
            }),
            BytecodeAssignmentTarget::ArrayIndexProperty {
                object,
                property,
                index,
                strict,
            } => Ok(BytecodeAssignmentReference::ArrayIndexProperty {
                object: self.eval_bytecode_expression(object)?,
                property: property.clone(),
                index: *index,
                strict: *strict,
            }),
            BytecodeAssignmentTarget::ComputedProperty {
                object,
                property,
                operand,
                strict,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                let property_value = self.eval_bytecode_expression(property)?;
                Self::require_bytecode_property_base(&object)?;
                let property = self.dynamic_property_key(&property_value)?;
                Ok(BytecodeAssignmentReference::ComputedProperty {
                    object,
                    property_value,
                    property: Some(property),
                    access: operand.access(),
                    strict: *strict,
                })
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                let name = self.resolve_private_name(property)?;
                Ok(BytecodeAssignmentReference::PrivateProperty { object, name })
            }
            BytecodeAssignmentTarget::SuperProperty { property, strict } => {
                self.eval_super_assignment_reference(property, *strict)
            }
        }
    }

    pub(in crate::runtime::bytecode) fn eval_bytecode_binding_assignment_reference(
        &mut self,
        name: &BytecodeBinding,
    ) -> Result<BytecodeAssignmentReference> {
        if let Some(reference) = self.resolve_with_binding(name)? {
            return Ok(BytecodeAssignmentReference::WithBinding {
                name: name.clone(),
                reference,
            });
        }
        let cell = self.get_or_materialize_binding_bytecode(name)?;
        if cell.is_none()
            && let Some(reference) = self.resolve_global_object_binding(name)?
        {
            return Ok(BytecodeAssignmentReference::WithBinding {
                name: name.clone(),
                reference,
            });
        }
        Ok(BytecodeAssignmentReference::Binding {
            name: name.clone(),
            cell,
        })
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
            BytecodeAssignmentTarget::WebCompatCall(target) => {
                self.eval_bytecode_expression(target)?;
                Err(web_compat_call_assignment_error())
            }
            BytecodeAssignmentTarget::StaticProperty {
                object,
                property,
                strict,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                self.set_bytecode_static_property_reference(
                    &object,
                    property.name(),
                    property.access(),
                    value,
                    *strict,
                )
            }
            BytecodeAssignmentTarget::ArrayIndexProperty {
                object,
                property,
                index,
                strict,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                self.set_bytecode_array_index_property(&object, property, *index, value, *strict)
            }
            BytecodeAssignmentTarget::ComputedProperty {
                object,
                property,
                operand,
                strict,
            } => {
                let object = self.eval_bytecode_expression(object)?;
                let property = self.eval_bytecode_expression(property)?;
                let mut property = self.dynamic_property_key(&property)?;
                self.set_bytecode_dynamic_property_reference(
                    &object,
                    &mut property,
                    operand.access(),
                    value,
                    *strict,
                )
            }
            BytecodeAssignmentTarget::PrivateProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                let name = self.resolve_private_name(property)?;
                self.write_private_slot(&object, &name, value)
            }
            BytecodeAssignmentTarget::SuperProperty { property, strict } => self
                .eval_super_assignment_reference(property, *strict)?
                .set(self, value),
        }
    }
}

const fn bytecode_set_failure(strict: bool) -> SetFailureBehavior {
    if strict {
        SetFailureBehavior::Throw
    } else {
        SetFailureBehavior::ReturnFalse
    }
}

fn logical_assignment_should_store(context: &Context, op: BinaryOp, value: &Value) -> Result<bool> {
    match op {
        BinaryOp::LogicalAnd => to_boolean(context, value),
        BinaryOp::LogicalOr => Ok(!to_boolean(context, value)?),
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
