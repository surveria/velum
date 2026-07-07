use std::rc::Rc;

use crate::{
    ast::{BinaryOp, DeclKind, StaticName, StaticPropertyAccessId, UnaryOp, UpdateOp},
    bytecode::{
        BytecodeAssignmentTarget, BytecodeBinding, BytecodeDynamicProperty,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
        BytecodeNumericUnaryOp,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::assertions::thrown_value_matches,
    runtime::call_args::RuntimeCallArgs,
    runtime::completion::Completion,
    runtime::numeric::{
        bitwise_and, bitwise_or, bitwise_xor, compare_binary, number_shift_count, number_to_i32,
        number_to_uint32, numeric_binary, shift_left, shift_right, shift_right_unsigned,
    },
    runtime::object::{OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyEnumerable},
    runtime::property::DynamicPropertyKey,
    value::{ErrorName, Value},
};

impl Context {
    pub(super) fn eval_bytecode_declaration(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        value: Option<Value>,
    ) -> Result<()> {
        match kind {
            DeclKind::Var => {
                if let Some(value) = value {
                    self.assign_bytecode(name, value)?;
                }
            }
            DeclKind::Let => {
                self.define_static(
                    name.name(),
                    value.unwrap_or(Value::Undefined),
                    DeclKind::Let,
                )?;
            }
            DeclKind::Const => {
                let Some(value) = value else {
                    return Err(Error::runtime("const declaration requires an initializer"));
                };
                self.define_static(name.name(), value, DeclKind::Const)?;
            }
        }
        Ok(())
    }

    pub(super) fn eval_bytecode_identifier(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.runtime_value(binding.value());
        }
        self.builtin_value(name.name().name())?
            .ok_or_else(|| crate::runtime::assertions::reference_error_undefined(name.name()))
    }

    pub(super) fn eval_bytecode_typeof_binding(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.heap_string_value(binding.value().type_name());
        }
        if let Some(value) = self.builtin_value(name.name().name())? {
            return self.heap_string_value(value.type_name());
        }
        self.heap_string_value(Value::Undefined.type_name())
    }

    pub(super) fn eval_bytecode_unary(op: UnaryOp, value: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::Bool(!value.is_truthy())),
            UnaryOp::Negate => value
                .as_number()
                .map(|value| Value::Number(-value))
                .ok_or_else(|| Error::runtime("unary '-' expects a number")),
            UnaryOp::Plus => value
                .as_number()
                .map(Value::Number)
                .ok_or_else(|| Error::runtime("unary '+' expects a number")),
            UnaryOp::Void => Ok(Value::Undefined),
            UnaryOp::Typeof | UnaryOp::Delete => Err(Error::runtime(
                "non-bytecode unary operator reached bytecode unary path",
            )),
        }
    }

    pub(super) fn eval_bytecode_number_unary(
        &self,
        op: BytecodeNumericUnaryOp,
        value: &Value,
    ) -> Result<Value> {
        if let Value::Number(value) = value {
            let value = match op {
                BytecodeNumericUnaryOp::Negate => -*value,
                BytecodeNumericUnaryOp::Plus => *value,
            };
            return self.checked_value(Value::Number(value));
        }
        Self::eval_bytecode_unary(op.fallback_unary(), value)
    }

    pub(super) fn eval_bytecode_binary(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        property_access: Option<BytecodeDynamicProperty>,
    ) -> Result<Value> {
        let value = match op {
            BinaryOp::Add => self.add(left, right)?,
            BinaryOp::Sub => numeric_binary(left, right, "-", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(left, right, "*", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(left, right, "/", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(left, right, "%", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(left, right, "**", f64::powf)?,
            BinaryOp::Equal | BinaryOp::StrictEqual => Value::Bool(left == right),
            BinaryOp::NotEqual | BinaryOp::StrictNotEqual => Value::Bool(left != right),
            BinaryOp::Less => compare_binary(left, right, "<", |left, right| left < right)?,
            BinaryOp::LessEqual => compare_binary(left, right, "<=", |left, right| left <= right)?,
            BinaryOp::Greater => compare_binary(left, right, ">", |left, right| left > right)?,
            BinaryOp::GreaterEqual => {
                compare_binary(left, right, ">=", |left, right| left >= right)?
            }
            BinaryOp::In => self.eval_bytecode_in(left, right, property_access)?,
            BinaryOp::BitAnd => bitwise_and(left, right)?,
            BinaryOp::BitOr => bitwise_or(left, right)?,
            BinaryOp::BitXor => bitwise_xor(left, right)?,
            BinaryOp::ShiftLeft => shift_left(left, right)?,
            BinaryOp::ShiftRight => shift_right(left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(left, right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                return Err(Error::runtime(
                    "logical operator reached bytecode eager evaluation",
                ));
            }
        };
        self.checked_value(value)
    }

    pub(super) fn eval_bytecode_number_binary(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
            let value = match op {
                BytecodeNumericBinaryOp::Add => left + right,
                BytecodeNumericBinaryOp::Sub => left - right,
                BytecodeNumericBinaryOp::Mul => left * right,
                BytecodeNumericBinaryOp::Div => left / right,
                BytecodeNumericBinaryOp::Rem => left % right,
                BytecodeNumericBinaryOp::Pow => left.powf(*right),
                BytecodeNumericBinaryOp::BitAnd => {
                    f64::from(number_to_i32(*left, "&")? & number_to_i32(*right, "&")?)
                }
                BytecodeNumericBinaryOp::BitOr => {
                    f64::from(number_to_i32(*left, "|")? | number_to_i32(*right, "|")?)
                }
                BytecodeNumericBinaryOp::BitXor => {
                    f64::from(number_to_i32(*left, "^")? ^ number_to_i32(*right, "^")?)
                }
                BytecodeNumericBinaryOp::ShiftLeft => {
                    let left = number_to_i32(*left, "<<")?;
                    let right = number_shift_count(*right, "<<")?;
                    f64::from(left.wrapping_shl(right))
                }
                BytecodeNumericBinaryOp::ShiftRight => {
                    let left = number_to_i32(*left, ">>")?;
                    let right = number_shift_count(*right, ">>")?;
                    f64::from(left.wrapping_shr(right))
                }
                BytecodeNumericBinaryOp::ShiftRightUnsigned => {
                    let left = number_to_uint32(*left, ">>>")?;
                    let right = number_shift_count(*right, ">>>")?;
                    f64::from(left.wrapping_shr(right))
                }
            };
            return self.checked_value(Value::Number(value));
        }
        self.eval_bytecode_binary(op.fallback_binary(), left, right, None)
    }

    pub(super) fn eval_bytecode_number_compare(
        &mut self,
        op: BytecodeNumericCompareOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
            let value = match op {
                BytecodeNumericCompareOp::Less => left < right,
                BytecodeNumericCompareOp::LessEqual => left <= right,
                BytecodeNumericCompareOp::Greater => left > right,
                BytecodeNumericCompareOp::GreaterEqual => left >= right,
            };
            return self.checked_value(Value::Bool(value));
        }
        self.eval_bytecode_binary(op.fallback_binary(), left, right, None)
    }

    pub(super) fn eval_bytecode_number_equality(
        &mut self,
        op: BytecodeNumericEqualityOp,
        left: &Value,
        right: &Value,
    ) -> Result<Value> {
        if let (Value::Number(left), Value::Number(right)) = (left, right) {
            let equal = bytecode_numbers_equal(*left, *right);
            let value = match op {
                BytecodeNumericEqualityOp::Equal | BytecodeNumericEqualityOp::StrictEqual => equal,
                BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual => {
                    !equal
                }
            };
            return self.checked_value(Value::Bool(value));
        }
        self.eval_bytecode_binary(op.fallback_binary(), left, right, None)
    }

    fn eval_bytecode_in(
        &self,
        left: &Value,
        right: &Value,
        property_access: Option<BytecodeDynamicProperty>,
    ) -> Result<Value> {
        let property = self.dynamic_property_key(left)?;
        if let Some(access) = property_access {
            return self
                .has_cached_dynamic_property_value(right, &property, access.access())
                .map(Value::Bool);
        }
        self.has_dynamic_property_value(right, &property)
            .map(Value::Bool)
    }

    pub(super) fn eval_bytecode_update_binding(
        &self,
        name: &BytecodeBinding,
        op: UpdateOp,
        prefix: bool,
    ) -> Result<Value> {
        let binding = self
            .get_binding_bytecode(name)?
            .ok_or_else(|| Error::runtime(format!("ReferenceError: '{name}' is not defined")))?;
        let old_value = binding.value();
        let new_value = Self::updated_bytecode_number(&old_value, op)?;
        self.checked_value(new_value.clone())?;
        binding.assign(name.name(), new_value.clone())?;
        Ok(if prefix { new_value } else { old_value })
    }

    pub(super) fn eval_bytecode_update_static_property(
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

    pub(super) fn eval_bytecode_update_dynamic_property(
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

    pub(super) fn updated_bytecode_number(value: &Value, op: UpdateOp) -> Result<Value> {
        let Some(number) = value.as_number() else {
            return Err(Error::runtime("update operator expects a number"));
        };
        let updated = match op {
            UpdateOp::Increment => number + 1.0,
            UpdateOp::Decrement => number - 1.0,
        };
        Ok(Value::Number(updated))
    }

    pub(super) fn eval_bytecode_binding_compound_assignment(
        &mut self,
        op: BinaryOp,
        name: &BytecodeBinding,
        right: &Value,
    ) -> Result<Value> {
        let binding = self
            .get_or_materialize_binding_bytecode(name)?
            .ok_or_else(|| Error::runtime(format!("ReferenceError: '{name}' is not defined")))?;
        let old_value = binding.value();
        let value = self.eval_bytecode_compound_value(op, &old_value, right)?;
        binding.assign(name.name(), value.clone())?;
        Ok(value)
    }

    pub(super) fn eval_bytecode_static_compound_assignment(
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

    pub(super) fn eval_bytecode_dynamic_compound_assignment(
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

    pub(super) fn eval_bytecode_compound_value(
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
            BinaryOp::Sub => numeric_binary(left, right, "-=", |left, right| left - right)?,
            BinaryOp::Mul => numeric_binary(left, right, "*=", |left, right| left * right)?,
            BinaryOp::Div => numeric_binary(left, right, "/=", |left, right| left / right)?,
            BinaryOp::Rem => numeric_binary(left, right, "%=", |left, right| left % right)?,
            BinaryOp::Pow => numeric_binary(left, right, "**=", f64::powf)?,
            BinaryOp::BitAnd => bitwise_and(left, right)?,
            BinaryOp::BitOr => bitwise_or(left, right)?,
            BinaryOp::BitXor => bitwise_xor(left, right)?,
            BinaryOp::ShiftLeft => shift_left(left, right)?,
            BinaryOp::ShiftRight => shift_right(left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(left, right)?,
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::StrictEqual
            | BinaryOp::StrictNotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::In
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr => {
                return Err(Error::runtime("invalid compound assignment operator"));
            }
        };
        self.runtime_value(value)
    }

    pub(super) fn assign_bytecode_target(
        &mut self,
        target: &BytecodeAssignmentTarget,
        value: Value,
    ) -> Result<()> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => self.assign_bytecode(name, value),
            BytecodeAssignmentTarget::StaticProperty { object, property } => {
                let object = self.eval_bytecode_expression(object)?;
                self.set_static_property_value(&object, property.name(), property.access(), value)
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
        }
    }

    pub(super) fn eval_bytecode_assert_throws(
        &mut self,
        expected: ErrorName,
        callback: &Value,
        message: Option<Value>,
    ) -> Result<Value> {
        if let Some(message) = message {
            self.runtime_value(message)?;
        }
        let Value::Function(id) = callback else {
            return Err(Error::runtime("assert.throws callback must be a function"));
        };
        let expected_name = expected.as_str();
        match self.eval_function_completion(*id, RuntimeCallArgs::values(&[]))? {
            Completion::Throw(value) if thrown_value_matches(&value, expected_name) => {
                Ok(Value::Undefined)
            }
            Completion::Throw(value) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, got {value}"
            ))),
            Completion::Normal(_) | Completion::Return(_) => Err(Error::runtime(format!(
                "assert.throws expected {expected_name}, but no exception was thrown"
            ))),
            completion @ (Completion::Break | Completion::Continue) => {
                completion.into_function_result()
            }
        }
    }

    pub(super) fn create_bytecode_object_literal(
        &mut self,
        properties: &Rc<[StaticName]>,
        values: Vec<Value>,
    ) -> Result<Value> {
        if properties.len() != values.len() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut inits = Vec::with_capacity(properties.len());
        for (property, value) in properties.iter().zip(values) {
            let key = self.intern_static_property_key(property)?;
            inits.push(ObjectPropertyInit::new(
                key,
                property.as_str(),
                value,
                PropertyEnumerable::Yes,
            ));
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}

const fn bytecode_numbers_equal(left: f64, right: f64) -> bool {
    if left.is_nan() || right.is_nan() {
        return false;
    }
    if bytecode_number_is_zero(left) && bytecode_number_is_zero(right) {
        return true;
    }
    left.to_bits() == right.to_bits()
}

const fn bytecode_number_is_zero(value: f64) -> bool {
    value.to_bits() << 1 == 0
}
