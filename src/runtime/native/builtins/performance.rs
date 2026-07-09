use crate::{
    error::Result,
    runtime::{
        Context,
        object::{PropertyConfigurable, PropertyEnumerable, PropertyWritable},
    },
    value::Value,
};

use super::{NativeFunctionKind, PERFORMANCE_NAME, PERFORMANCE_NOW_NAME};

impl Context {
    pub(in crate::runtime::native) fn performance_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(PERFORMANCE_NAME) {
            let value = binding.value(PERFORMANCE_NAME)?;
            self.define_performance_global_property(value.clone())?;
            return Ok(value);
        }

        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let now = self.create_ephemeral_native_function(
            NativeFunctionKind::PerformanceNow,
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(object, PERFORMANCE_NOW_NAME, now)?;

        let value = Value::Object(object);
        self.insert_global_builtin(PERFORMANCE_NAME, value.clone())?;
        self.define_performance_global_property(value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime) fn eval_performance_now(&mut self) -> Value {
        Value::Number(self.performance_now_millis())
    }

    fn define_performance_global_property(&mut self, value: Value) -> Result<()> {
        let global = self.global_object_id()?;
        self.define_global_object_data_property(
            global,
            PERFORMANCE_NAME,
            value,
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        )
    }
}
