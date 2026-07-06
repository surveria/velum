use crate::{
    ast::Expr,
    error::{Error, Result},
    runtime::Context,
    runtime_object::{ObjectPropertyInit, PropertyEnumerable},
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{BOOLEAN_NAME, NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

impl Context {
    pub(super) fn boolean_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Boolean) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.boolean_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Boolean, prototype));
        self.insert_global_builtin(BOOLEAN_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_boolean_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_boolean_argument(args).map(Value::Bool)
    }

    pub(super) fn construct_boolean_object(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_boolean_argument(args)?;
        let prototype = self.boolean_constructor_prototype()?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn boolean_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn boolean_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.boolean_constructor_value()? else {
            return Err(Error::runtime("Boolean constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Boolean prototype is not an object")),
        }
    }

    fn eval_boolean_argument(&mut self, args: &[Expr]) -> Result<bool> {
        let value = self.eval_native_unary_argument_value(args)?;
        Ok(value.as_ref().is_some_and(Value::is_truthy))
    }
}
