use crate::{
    ast::Expr,
    error::{Error, Result},
    runtime::Context,
    runtime_object::{ObjectPropertyInit, PropertyEnumerable},
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, STRING_NAME};

const STRING_LENGTH_PROPERTY: &str = "length";

impl Context {
    pub(super) fn string_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::String) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.string_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::String)?;
        self.native_functions.push(NativeFunction::new(
            NativeFunctionKind::String,
            prototype,
            name,
        ));
        self.insert_global_builtin(STRING_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_string_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let value = self.eval_string_argument(args)?;
        self.heap_string_value(&value)
    }

    pub(super) fn construct_string_object(&mut self, args: &[Expr]) -> Result<Value> {
        let value = self.eval_string_argument(args)?;
        let value = self.intern_heap_string(&value)?;
        let prototype = self.string_constructor_prototype()?;
        let length_key = self.intern_property_key(STRING_LENGTH_PROPERTY)?;
        self.objects.create_string_object(
            value,
            prototype,
            length_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn string_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn string_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.string_constructor_value()? else {
            return Err(Error::runtime("String constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("String prototype is not an object")),
        }
    }

    fn eval_string_argument(&mut self, args: &[Expr]) -> Result<String> {
        let value = self.eval_native_unary_argument_value(args)?;
        let value = Self::string_argument_value(value.as_ref());
        self.check_string_len(&value)?;
        Ok(value)
    }

    fn string_argument_value(value: Option<&Value>) -> String {
        value.map_or_else(String::new, ToString::to_string)
    }
}
