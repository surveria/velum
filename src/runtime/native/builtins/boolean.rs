use crate::{
    error::{Error, Result},
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPrimitiveValue, PropertyLookup},
    runtime::{Context, abstract_operations::to_boolean},
    value::{ObjectId, Value},
};

use super::{
    BOOLEAN_NAME, BOOLEAN_PROTOTYPE_TO_STRING_NAME, BOOLEAN_PROTOTYPE_VALUE_OF_NAME,
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
};

const BOOLEAN_VALUE_RECEIVER_ERROR: &str =
    "Boolean.prototype value method requires a boolean or Boolean object";

impl Context {
    pub(in crate::runtime::native) fn boolean_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Boolean) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.boolean_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::Boolean)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Boolean, prototype, name)?;
        self.install_boolean_prototype_methods(prototype_id)?;
        self.insert_global_builtin(BOOLEAN_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_boolean_constructor(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_boolean_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_boolean_constructor(
        &self,
        args: &[Value],
    ) -> Result<Value> {
        self.checked_value(Value::Bool(self.eval_boolean_argument(args)?))
    }

    pub(in crate::runtime::native) fn construct_boolean_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.eval_boolean_argument(args.as_slice())?;
        let prototype = self.boolean_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Bool(value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime::native) fn create_boolean_object_from_value(
        &mut self,
        value: bool,
    ) -> Result<Value> {
        let prototype = self.boolean_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Bool(value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime::native) fn eval_boolean_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_boolean_extra_args(args.as_slice());
        self.eval_direct_boolean_prototype_to_string(this_value)
    }

    pub(in crate::runtime) fn eval_direct_boolean_prototype_to_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.boolean_receiver_value(this_value)?;
        self.heap_string_value(if value { "true" } else { "false" })
    }

    pub(in crate::runtime::native) fn eval_boolean_prototype_value_of(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_boolean_extra_args(args.as_slice());
        self.eval_direct_boolean_prototype_value_of(this_value)
    }

    pub(in crate::runtime) fn eval_direct_boolean_prototype_value_of(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.boolean_receiver_value(this_value).map(Value::Bool)
    }

    pub(in crate::runtime) fn boolean_prototype_property_value(
        &mut self,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.boolean_constructor_prototype()?;
        self.get_prototype_property_value_with_receiver(prototype, receiver, property)
    }

    pub(in crate::runtime) fn boolean_prototype_property_value_with_lookup(
        &mut self,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let prototype = self.boolean_constructor_prototype()?;
        self.get_prototype_property_value_with_lookup(prototype, receiver, property)
    }

    fn boolean_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Bool(false),
            object_prototype,
            self.limits.max_objects,
        )?
        else {
            return Err(Error::runtime("Boolean prototype is not an object"));
        };
        self.define_non_enumerable_object_property(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        Ok(prototype)
    }

    pub(in crate::runtime) fn boolean_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.boolean_constructor_value()? else {
            return Err(Error::runtime("Boolean constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Boolean prototype is not an object")),
        }
    }

    fn install_boolean_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        self.define_boolean_prototype_method(
            prototype,
            BOOLEAN_PROTOTYPE_TO_STRING_NAME,
            NativeFunctionKind::BooleanPrototypeToString,
        )?;
        self.define_boolean_prototype_method(
            prototype,
            BOOLEAN_PROTOTYPE_VALUE_OF_NAME,
            NativeFunctionKind::BooleanPrototypeValueOf,
        )
    }

    fn define_boolean_prototype_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, function)
    }

    fn eval_boolean_argument(&self, args: &[Value]) -> Result<bool> {
        let Some(value) = args.first() else {
            return Ok(false);
        };
        to_boolean(self, value)
    }

    fn boolean_receiver_value(&self, value: &Value) -> Result<bool> {
        match value {
            Value::Bool(value) => Ok(*value),
            Value::Object(id) => match self.objects.primitive_value(*id)? {
                Some(ObjectPrimitiveValue::Bool(value)) => Ok(*value),
                Some(
                    ObjectPrimitiveValue::Number(_)
                    | ObjectPrimitiveValue::BigInt(_)
                    | ObjectPrimitiveValue::Symbol(_),
                )
                | None => Err(Error::type_error(BOOLEAN_VALUE_RECEIVER_ERROR)),
            },
            _ => Err(Error::type_error(BOOLEAN_VALUE_RECEIVER_ERROR)),
        }
    }

    const fn discard_boolean_extra_args(_args: &[Value]) {}
}
