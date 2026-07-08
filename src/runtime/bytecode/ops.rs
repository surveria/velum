use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeArrayIndex, BytecodeAssignmentTarget, BytecodeBinding, BytecodeBlock,
        BytecodeDynamicProperty, BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
        BytecodeNumericEqualityOp, BytecodeNumericUnaryOp, BytecodeObjectProperty,
        BytecodeProperty,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
    runtime::bytecode::coercion::{abstract_equality, relational_compare, strict_equality},
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    runtime::control::thrown_value_matches,
    runtime::native::NativeFunctionKind,
    runtime::numeric::{
        bitwise_and, bitwise_or, bitwise_xor, number_shift_count, number_to_i32, number_to_uint32,
        numeric_binary, shift_left, shift_right, shift_right_unsigned,
    },
    runtime::object::{
        DataPropertyUpdate, OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyWritable,
    },
    runtime::property::DynamicPropertyKey,
    syntax::{
        AccessorKind, BinaryOp, DeclKind, StaticName, StaticPropertyAccessId, UnaryOp, UpdateOp,
    },
    value::{ErrorName, Value},
};

const INSTANCEOF_PROTOTYPE_PROPERTY: &str = "prototype";
const INSTANCEOF_NOT_CALLABLE_ERROR: &str = "right-hand side of 'instanceof' is not callable";
const INSTANCEOF_NON_OBJECT_PROTOTYPE_ERROR: &str =
    "right-hand side of 'instanceof' has non-object prototype";
const FUNCTION_NAME_PROPERTY: &str = "name";

enum BytecodeAssignmentReference {
    Binding {
        name: BytecodeBinding,
        cell: BindingCell,
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
}

impl BytecodeAssignmentReference {
    fn get(&self, context: &mut Context) -> Result<Value> {
        match self {
            Self::Binding { name, cell } => cell.value(name.name()),
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
        }
    }

    fn set(&self, context: &mut Context, value: Value) -> Result<()> {
        match self {
            Self::Binding { name, cell } => {
                let value = context.runtime_value(value)?;
                cell.assign(name.name(), value)
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
        }
    }
}

fn logical_assignment_should_store(op: BinaryOp, value: &Value) -> Result<bool> {
    match op {
        BinaryOp::LogicalAnd => Ok(value.is_truthy()),
        BinaryOp::LogicalOr => Ok(!value.is_truthy()),
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
        if let Some(value) = self.unresolved_builtin_numeric_constant(name) {
            return Ok(value);
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.runtime_value(binding.value(name.name())?);
        }
        self.builtin_value(name.name().name())?
            .ok_or_else(|| crate::runtime::control::reference_error_undefined(name.name()))
    }

    pub(super) fn eval_bytecode_typeof_binding(&mut self, name: &BytecodeBinding) -> Result<Value> {
        if self.unresolved_builtin_numeric_constant(name).is_some() {
            return self.heap_string_value(Value::Number(0.0).type_name());
        }
        if let Some(binding) = self.get_binding_bytecode(name)? {
            return self.heap_string_value(binding.value(name.name())?.type_name());
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
        Self::eval_bytecode_unary(op.generic_unary(), value)
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
            BinaryOp::Equal => Value::Bool(abstract_equality(self, left, right)?),
            BinaryOp::NotEqual => Value::Bool(!abstract_equality(self, left, right)?),
            BinaryOp::StrictEqual => Value::Bool(strict_equality(left, right)),
            BinaryOp::StrictNotEqual => Value::Bool(!strict_equality(left, right)),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                relational_compare(op, left, right)
            }
            BinaryOp::In => self.eval_bytecode_in(left, right, property_access)?,
            BinaryOp::InstanceOf => self.eval_bytecode_instanceof(left, right)?,
            BinaryOp::BitAnd => bitwise_and(left, right)?,
            BinaryOp::BitOr => bitwise_or(left, right)?,
            BinaryOp::BitXor => bitwise_xor(left, right)?,
            BinaryOp::ShiftLeft => shift_left(left, right)?,
            BinaryOp::ShiftRight => shift_right(left, right)?,
            BinaryOp::ShiftRightUnsigned => shift_right_unsigned(left, right)?,
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing => {
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
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
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
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
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
        self.eval_bytecode_binary(op.generic_binary(), left, right, None)
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

    fn eval_bytecode_instanceof(&mut self, left: &Value, right: &Value) -> Result<Value> {
        let target = self.instanceof_target_prototype(right)?;
        let matches = if let Value::Error(error) = left {
            self.error_matches_instanceof(error.name(), right)?
        } else {
            self.value_prototype_chain_has_object(left, target)?
        };
        Ok(Value::Bool(matches))
    }

    fn instanceof_target_prototype(&mut self, right: &Value) -> Result<crate::value::ObjectId> {
        if !Self::is_callable(right) {
            return Err(Error::runtime(INSTANCEOF_NOT_CALLABLE_ERROR));
        }
        let prototype = match right {
            Value::Function(id) => self.get_function_property_lookup(
                *id,
                self.property_lookup(INSTANCEOF_PROTOTYPE_PROPERTY),
            )?,
            Value::NativeFunction(id) => self.get_native_function_property_lookup(
                *id,
                self.property_lookup(INSTANCEOF_PROTOTYPE_PROPERTY),
            )?,
            Value::HostFunction(_) => Value::Undefined,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Object(_)
            | Value::Error(_) => return Err(Error::runtime(INSTANCEOF_NOT_CALLABLE_ERROR)),
        };
        let Value::Object(id) = prototype else {
            return Err(Error::runtime(INSTANCEOF_NON_OBJECT_PROTOTYPE_ERROR));
        };
        Ok(id)
    }

    fn value_prototype_chain_has_object(
        &mut self,
        value: &Value,
        target: crate::value::ObjectId,
    ) -> Result<bool> {
        match value {
            Value::Object(id) => self.objects.prototype_chain_has_object(*id, target),
            Value::Function(id) => {
                let prototype = self.function_object_prototype_value(*id)?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::NativeFunction(id) => {
                let prototype = self.native_function_object_prototype_value(*id)?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::HostFunction(_) => {
                let prototype = self.function_constructor_prototype_value()?;
                self.prototype_value_chain_has_object(&prototype, target)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Error(_) => Ok(false),
        }
    }

    fn prototype_value_chain_has_object(
        &self,
        prototype: &Value,
        target: crate::value::ObjectId,
    ) -> Result<bool> {
        let Value::Object(id) = prototype else {
            return Ok(false);
        };
        if *id == target {
            return Ok(true);
        }
        self.objects.prototype_chain_has_object(*id, target)
    }

    fn error_matches_instanceof(&self, name: ErrorName, right: &Value) -> Result<bool> {
        let Value::NativeFunction(id) = right else {
            return Ok(false);
        };
        let NativeFunctionKind::ErrorConstructor(expected) = self.native_function(*id)?.kind()
        else {
            return Ok(false);
        };
        if expected == ErrorName::Base {
            return Ok(name.is_standard());
        }
        Ok(name == expected)
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
        let old_value = binding.value(name.name())?;
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
        let old_value = binding.value(name.name())?;
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
            | BinaryOp::InstanceOf
            | BinaryOp::LogicalAnd
            | BinaryOp::LogicalOr
            | BinaryOp::NullishCoalescing => {
                return Err(Error::runtime("invalid compound assignment operator"));
            }
        };
        self.runtime_value(value)
    }

    pub(super) fn eval_bytecode_logical_assignment(
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

    fn eval_bytecode_assignment_reference(
        &mut self,
        target: &BytecodeAssignmentTarget,
    ) -> Result<BytecodeAssignmentReference> {
        match target {
            BytecodeAssignmentTarget::Binding(name) => {
                let cell = self
                    .get_or_materialize_binding_bytecode(name)?
                    .ok_or_else(|| {
                        Error::runtime(format!("ReferenceError: '{name}' is not defined"))
                    })?;
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
        }
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
            completion @ (Completion::Break { .. } | Completion::Continue(_)) => {
                completion.into_function_result()
            }
        }
    }

    pub(super) fn create_bytecode_object_literal(
        &mut self,
        properties: &Rc<[BytecodeObjectProperty]>,
        values: Vec<Value>,
    ) -> Result<Value> {
        if object_literal_stack_value_count(properties)? != values.len() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut values = values.into_iter();
        let mut dynamic_names = Vec::new();
        let mut entries = Vec::with_capacity(properties.len());
        for property in properties.iter() {
            match property {
                BytecodeObjectProperty::Static(name) => {
                    let value = next_object_literal_stack_value(&mut values)?;
                    let key = self.intern_static_property_key(name)?;
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Static(name.as_str()),
                        value,
                        accessor: None,
                    });
                }
                BytecodeObjectProperty::StaticAccessor { key: name, kind } => {
                    let value = next_object_literal_stack_value(&mut values)?;
                    let key = self.intern_static_property_key(name)?;
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Static(name.as_str()),
                        value,
                        accessor: Some(*kind),
                    });
                }
                BytecodeObjectProperty::Spread => {
                    let source = next_object_literal_stack_value(&mut values)?;
                    self.push_spread_literal_entries(&source, &mut dynamic_names, &mut entries)?;
                }
                BytecodeObjectProperty::Computed
                | BytecodeObjectProperty::ComputedMethod
                | BytecodeObjectProperty::ComputedAccessor { .. } => {
                    let set_method_name = matches!(
                        property,
                        BytecodeObjectProperty::ComputedMethod
                            | BytecodeObjectProperty::ComputedAccessor { .. }
                    );
                    let accessor = match property {
                        BytecodeObjectProperty::ComputedAccessor { kind } => Some(*kind),
                        _ => None,
                    };
                    let key_value = next_object_literal_stack_value(&mut values)?;
                    let value = next_object_literal_stack_value(&mut values)?;
                    let mut property = self.dynamic_property_key(&key_value)?;
                    let key = self.intern_dynamic_property_key(&mut property)?;
                    if set_method_name {
                        self.set_computed_method_name(&value, property.name())?;
                    }
                    let name_index = dynamic_names.len();
                    dynamic_names.push(property.name().to_owned());
                    entries.push(RuntimeObjectLiteralEntry {
                        key,
                        name: RuntimeObjectLiteralName::Dynamic(name_index),
                        value,
                        accessor,
                    });
                }
            }
        }
        if values.next().is_some() {
            return Err(Error::runtime(
                "bytecode object literal stack arity mismatch",
            ));
        }
        let mut inits = Vec::with_capacity(entries.len());
        for entry in entries {
            let is_dynamic = entry.name.is_dynamic();
            let name = match entry.name {
                RuntimeObjectLiteralName::Static(name) => name,
                RuntimeObjectLiteralName::Dynamic(index) => dynamic_names
                    .get(index)
                    .map(String::as_str)
                    .ok_or_else(|| Error::runtime("computed object property name disappeared"))?,
            };
            let init = if let Some(kind) = entry.accessor {
                ObjectPropertyInit::new_accessor(entry.key, name, entry.value, kind)
            } else if is_dynamic {
                ObjectPropertyInit::new_data(entry.key, name, entry.value, PropertyEnumerable::Yes)
            } else {
                ObjectPropertyInit::new(entry.key, name, entry.value, PropertyEnumerable::Yes)
            };
            inits.push(init);
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    /// Copies own enumerable properties of a spread source into pending
    /// object-literal entries; null and undefined sources copy nothing.
    fn push_spread_literal_entries(
        &mut self,
        source: &Value,
        dynamic_names: &mut Vec<String>,
        entries: &mut Vec<RuntimeObjectLiteralEntry<'_>>,
    ) -> Result<()> {
        if matches!(source, Value::Undefined | Value::Null) {
            return Ok(());
        }
        for key in self.own_enumerable_keys(source)? {
            let value = self.get_property_value(source, &key)?;
            let property_key = self.intern_property_key(&key)?;
            let name_index = dynamic_names.len();
            dynamic_names.push(key);
            entries.push(RuntimeObjectLiteralEntry {
                key: property_key,
                name: RuntimeObjectLiteralName::Dynamic(name_index),
                value,
                accessor: None,
            });
        }
        Ok(())
    }

    pub(super) fn set_computed_method_name(&mut self, value: &Value, name: &str) -> Result<()> {
        let Value::Function(id) = value else {
            return Ok(());
        };
        let key = self.intern_property_key(FUNCTION_NAME_PROPERTY)?;
        let value = self.heap_string_value(name)?;
        self.define_function_property_key(
            *id,
            FUNCTION_NAME_PROPERTY,
            key,
            DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum RuntimeObjectLiteralName<'a> {
    Static(&'a str),
    Dynamic(usize),
}

impl RuntimeObjectLiteralName<'_> {
    const fn is_dynamic(self) -> bool {
        matches!(self, Self::Dynamic(_))
    }
}

#[derive(Debug)]
struct RuntimeObjectLiteralEntry<'a> {
    key: PropertyKey,
    name: RuntimeObjectLiteralName<'a>,
    value: Value,
    accessor: Option<AccessorKind>,
}

fn object_literal_stack_value_count(properties: &[BytecodeObjectProperty]) -> Result<usize> {
    let mut count = 0_usize;
    for property in properties {
        count = count
            .checked_add(property.stack_value_count())
            .ok_or_else(|| Error::limit("object literal stack value count overflowed"))?;
    }
    Ok(count)
}

fn next_object_literal_stack_value(values: &mut impl Iterator<Item = Value>) -> Result<Value> {
    values
        .next()
        .ok_or_else(|| Error::runtime("bytecode object literal stack arity mismatch"))
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
