use crate::{
    error::Result,
    runtime::{
        Context,
        call::RuntimeCallArgs,
        control::runtime_exception_value,
        object::{ObjectPropertyInit, PropertyEnumerable},
        roots::VmRootKind,
    },
    value::Value,
};

const PROMISE_PROPERTY: &str = "promise";
const RESOLVE_PROPERTY: &str = "resolve";
const REJECT_PROPERTY: &str = "reject";

impl Context {
    pub(super) fn eval_promise_try(
        &mut self,
        args: RuntimeCallArgs<'_>,
        constructor: &Value,
    ) -> Result<Value> {
        let capability = self.new_promise_capability(constructor)?;
        let values = args.as_slice();
        let callback = values.first().cloned().unwrap_or(Value::Undefined);
        let callback_args = values.get(1..).unwrap_or_default();
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            capability
                .root_values()
                .into_iter()
                .chain(std::iter::once(&callback))
                .chain(callback_args.iter()),
        )?;
        match self.call_value(&callback, callback_args, Value::Undefined) {
            Ok(result) => {
                self.call_value(&capability.resolve, &[result], Value::Undefined)?;
            }
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.call_value(&capability.reject, &[reason], Value::Undefined)?;
            }
        }
        Ok(capability.promise)
    }

    pub(super) fn eval_promise_with_resolvers(&mut self, constructor: &Value) -> Result<Value> {
        let capability = self.new_promise_capability(constructor)?;
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, capability.root_values())?;
        let promise_key = self.intern_property_key(PROMISE_PROPERTY)?;
        let resolve_key = self.intern_property_key(RESOLVE_PROPERTY)?;
        let reject_key = self.intern_property_key(REJECT_PROPERTY)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            vec![
                ObjectPropertyInit::new(
                    promise_key,
                    PROMISE_PROPERTY,
                    capability.promise,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(
                    resolve_key,
                    RESOLVE_PROPERTY,
                    capability.resolve,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(
                    reject_key,
                    REJECT_PROPERTY,
                    capability.reject,
                    PropertyEnumerable::Yes,
                ),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
