use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, object::OwnPropertyDescriptor, object::PropertyEnumerable,
        object::PropertyKey, object::PropertyUpdate,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{NativeFunctionKind, PROXY_REVOCABLE_NAME};

const PROXY_REVOCABLE_PROXY_PROPERTY: &str = "proxy";
const PROXY_REVOCABLE_REVOKE_PROPERTY: &str = "revoke";

const PROXY_TARGET_NOT_OBJECT_ERROR: &str = "Proxy target must be an object";
const PROXY_HANDLER_NOT_OBJECT_ERROR: &str = "Proxy handler must be an object";
const PROXY_REQUIRES_NEW_ERROR: &str = "Constructor Proxy requires 'new'";
const PROXY_REVOKED_ERROR: &str = "Cannot perform operation on a revoked Proxy";
const PROXY_TRAP_NOT_CALLABLE_ERROR: &str = "Proxy handler trap is not callable";
const PROXY_TRAP_GET: &str = "get";
const PROXY_TRAP_SET: &str = "set";
const PROXY_TRAP_HAS: &str = "has";
const PROXY_TRAP_DELETE: &str = "deleteProperty";
const PROXY_TRAP_GET_PROTOTYPE_OF: &str = "getPrototypeOf";
const PROXY_TRAP_SET_PROTOTYPE_OF: &str = "setPrototypeOf";
const PROXY_TRAP_IS_EXTENSIBLE: &str = "isExtensible";
const PROXY_TRAP_PREVENT_EXTENSIONS: &str = "preventExtensions";
const PROXY_TRAP_DEFINE_PROPERTY: &str = "defineProperty";
const PROXY_TRAP_OWN_KEYS: &str = "ownKeys";
const PROXY_TRAP_GET_OWN_DESCRIPTOR: &str = "getOwnPropertyDescriptor";
const PROXY_DESCRIPTOR_INVALID_ERROR: &str =
    "proxy getOwnPropertyDescriptor trap must return an object or undefined";
const PROXY_TRAP_APPLY: &str = "apply";
const PROXY_TRAP_CONSTRUCT: &str = "construct";
const PROXY_CONSTRUCT_INVALID_ERROR: &str = "proxy construct trap must return an object";
const PROXY_GET_PROTOTYPE_INVALID_ERROR: &str =
    "proxy getPrototypeOf trap must return an object or null";

impl Context {
    pub(in crate::runtime) fn proxy_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Proxy) {
            return Ok(Value::NativeFunction(id));
        }
        let constructor = self.create_native_function(NativeFunctionKind::Proxy, Value::Undefined)?;
        if let Value::NativeFunction(id) = constructor {
            self.define_proxy_static_method(
                id,
                PROXY_REVOCABLE_NAME,
                NativeFunctionKind::ProxyRevocable,
            )?;
        }
        self.insert_global_builtin(NativeFunctionKind::Proxy.name(), constructor.clone())?;
        Ok(constructor)
    }

    fn define_proxy_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No);
        Ok(())
    }

    /// Spec `Proxy(target, handler)` / `ProxyCreate`. Both operands must be
    /// objects (ordinary objects or callables). Calling `Proxy` without `new`
    /// is a `TypeError`.
    pub(in crate::runtime) fn eval_proxy_call(&mut self, _args: RuntimeCallArgs<'_>) -> Result<Value> {
        Err(Error::type_error(PROXY_REQUIRES_NEW_ERROR))
    }

    /// Spec `Proxy.revocable(target, handler)`: create a proxy plus a revoke
    /// function and return `{ proxy, revoke }`.
    pub(in crate::runtime) fn eval_proxy_revocable(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let proxy = self.construct_proxy_object(args)?;
        let Value::Object(proxy_id) = proxy else {
            return Err(Error::runtime("proxy construction did not yield an object"));
        };
        let revoke = self.create_ephemeral_native_function(
            NativeFunctionKind::ProxyRevoke(proxy_id),
            Value::Undefined,
        )?;
        let result = self.create_object_from_constructor()?;
        self.create_proxy_data_property(&result, PROXY_REVOCABLE_PROXY_PROPERTY, proxy)?;
        self.create_proxy_data_property(&result, PROXY_REVOCABLE_REVOKE_PROPERTY, revoke)?;
        Ok(result)
    }

    /// The revoke function returned by `Proxy.revocable`: disconnect the proxy
    /// from its target and handler.
    pub(in crate::runtime) fn eval_proxy_revoke(&mut self, id: ObjectId) -> Result<Value> {
        self.objects.revoke_proxy(id)?;
        Ok(Value::Undefined)
    }

    fn create_proxy_data_property(
        &mut self,
        object: &Value,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let key = PropertyKey::new(self.intern_atom(name)?);
        self.set_property_value_with_accessors(object, key, name, value)
    }

    pub(in crate::runtime) fn construct_proxy_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        let handler = Self::argument_or_undefined(slice.get(1));
        if !Self::is_object_like(&target) {
            return Err(Error::type_error(PROXY_TARGET_NOT_OBJECT_ERROR));
        }
        if !Self::is_object_like(&handler) {
            return Err(Error::type_error(PROXY_HANDLER_NOT_OBJECT_ERROR));
        }
        self.objects
            .create_proxy_object(target, handler, self.limits.max_objects)
    }

    /// A value counts as an object for Proxy internal slots when it is an
    /// ordinary object or any callable object.
    pub(in crate::runtime) const fn is_object_like(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Error(_)
        )
    }

    /// Resolve the wrapped target and handler for a proxy object, raising a
    /// `TypeError` when the proxy has been revoked.
    fn proxy_target_handler(&self, id: ObjectId) -> Result<(Value, Value)> {
        let proxy = self
            .objects
            .proxy_value(id)?
            .ok_or_else(|| Error::runtime("object is not a proxy"))?;
        let (Some(target), Some(handler)) = (proxy.target(), proxy.handler()) else {
            return Err(Error::type_error(PROXY_REVOKED_ERROR));
        };
        Ok((target.clone(), handler.clone()))
    }

    /// Spec `GetMethod(handler, name)`: return the trap if present and callable,
    /// `None` when it is `undefined`/`null`, and a `TypeError` otherwise.
    fn proxy_trap(&mut self, handler: &Value, name: &str) -> Result<Option<Value>> {
        let trap = self.get_property_value(handler, name)?;
        if matches!(trap, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        if !Self::is_callable(&trap) {
            return Err(Error::type_error(PROXY_TRAP_NOT_CALLABLE_ERROR));
        }
        Ok(Some(trap))
    }

    /// Proxy `[[Get]]`: dispatch to the `get` trap or fall back to the target.
    pub(in crate::runtime) fn proxy_get(
        &mut self,
        id: ObjectId,
        name: &str,
        receiver: Value,
    ) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_GET)? else {
            return self.get_property_value(&target, name);
        };
        let key = self.heap_string_value(name)?;
        self.eval_call_value(trap, &[target, key, receiver], handler)
    }

    /// Proxy `[[Has]]`: dispatch to the `has` trap or fall back to the target.
    pub(in crate::runtime) fn proxy_has(&mut self, id: ObjectId, name: &str) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_HAS)? else {
            let key_value = self.heap_string_value(name)?;
            let key = self.dynamic_property_key(&key_value)?;
            return self.has_dynamic_property_value(&target, &key);
        };
        let key = self.heap_string_value(name)?;
        let result = self.eval_call_value(trap, &[target, key], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[Set]]`: dispatch to the `set` trap or fall back to the target.
    /// Returns whether the assignment succeeded.
    pub(in crate::runtime) fn proxy_set(
        &mut self,
        id: ObjectId,
        name: &str,
        value: Value,
        receiver: Value,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_SET)? else {
            let property_key = crate::runtime::object::PropertyKey::new(self.intern_atom(name)?);
            self.set_property_value_with_accessors(&target, property_key, name, value)?;
            return Ok(true);
        };
        let key = self.heap_string_value(name)?;
        let result = self.eval_call_value(trap, &[target, key, value, receiver], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[Delete]]`: dispatch to the `deleteProperty` trap or fall back to
    /// the target. Returns whether the deletion succeeded.
    pub(in crate::runtime) fn proxy_delete(&mut self, id: ObjectId, name: &str) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_DELETE)? else {
            let key_value = self.heap_string_value(name)?;
            let key = self.dynamic_property_key(&key_value)?;
            return crate::runtime::property::delete_property(
                &mut self.objects,
                &target,
                key.lookup(),
            );
        };
        let key = self.heap_string_value(name)?;
        let result = self.eval_call_value(trap, &[target, key], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[GetPrototypeOf]]`.
    pub(in crate::runtime) fn proxy_get_prototype_of(&mut self, id: ObjectId) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_GET_PROTOTYPE_OF)? else {
            return self.eval_object_get_prototype_of(RuntimeCallArgs::values(&[target]));
        };
        let result = self.eval_call_value(trap, &[target], handler)?;
        if !matches!(result, Value::Object(_) | Value::Null) {
            return Err(Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR));
        }
        Ok(result)
    }

    /// Proxy `[[SetPrototypeOf]]`.
    pub(in crate::runtime) fn proxy_set_prototype_of(
        &mut self,
        id: ObjectId,
        prototype: Value,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_SET_PROTOTYPE_OF)? else {
            self.eval_direct_object_set_prototype_of(&[target, prototype])?;
            return Ok(true);
        };
        let result = self.eval_call_value(trap, &[target, prototype], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[IsExtensible]]`.
    pub(in crate::runtime) fn proxy_is_extensible(&mut self, id: ObjectId) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_IS_EXTENSIBLE)? else {
            return Ok(self
                .eval_direct_object_is_extensible(&[target])?
                .is_truthy());
        };
        let result = self.eval_call_value(trap, &[target], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[PreventExtensions]]`.
    pub(in crate::runtime) fn proxy_prevent_extensions(&mut self, id: ObjectId) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_PREVENT_EXTENSIONS)? else {
            self.eval_direct_object_prevent_extensions(&[target])?;
            return Ok(true);
        };
        let result = self.eval_call_value(trap, &[target], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[DefineOwnProperty]]`: dispatch the `defineProperty` trap or
    /// define the property on the target. Returns whether the definition
    /// succeeded.
    pub(in crate::runtime) fn proxy_define_property(
        &mut self,
        id: ObjectId,
        name: &str,
        descriptor: Value,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let key = self.heap_string_value(name)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_DEFINE_PROPERTY)? else {
            self.eval_object_define_property(RuntimeCallArgs::values(&[target, key, descriptor]))?;
            return Ok(true);
        };
        let result = self.eval_call_value(trap, &[target, key, descriptor], handler)?;
        Ok(result.is_truthy())
    }

    /// Proxy `[[OwnPropertyKeys]]`: dispatch the `ownKeys` trap or read the
    /// target's own string keys. Returns the trap's string keys (symbol keys
    /// are not yet surfaced).
    pub(in crate::runtime) fn proxy_own_keys(&mut self, id: ObjectId) -> Result<Vec<String>> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_OWN_KEYS)? else {
            return self.own_property_names(&target);
        };
        let result = self.eval_call_value(trap, &[target], handler)?;
        self.proxy_key_list_from_value(&result)
    }

    /// Proxy `[[GetOwnProperty]]`: dispatch the `getOwnPropertyDescriptor` trap
    /// or read the target's own descriptor.
    pub(in crate::runtime) fn proxy_get_own_property_descriptor(
        &mut self,
        id: ObjectId,
        name: &str,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let key_value = self.heap_string_value(name)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_GET_OWN_DESCRIPTOR)? else {
            let key = self.dynamic_property_key(&key_value)?;
            return self.own_property_descriptor_value(&target, &key);
        };
        let result = self.eval_call_value(trap, &[target, key_value], handler)?;
        match result {
            Value::Undefined => Ok(None),
            Value::Object(_) => Ok(Some(self.own_property_descriptor_from_object(&result)?)),
            _ => Err(Error::type_error(PROXY_DESCRIPTOR_INVALID_ERROR)),
        }
    }

    /// Spec `ToPropertyDescriptor` completed with the defineProperty defaults,
    /// producing a full own-descriptor snapshot from a descriptor object.
    fn own_property_descriptor_from_object(
        &mut self,
        descriptor: &Value,
    ) -> Result<OwnPropertyDescriptor> {
        Ok(match self.property_update_from_value(descriptor)? {
            PropertyUpdate::Data(data) => OwnPropertyDescriptor::Data(data.complete_for_new()),
            PropertyUpdate::Accessor(accessor) => {
                OwnPropertyDescriptor::Accessor(accessor.complete_for_new())
            }
        })
    }

    /// Proxy `[[Call]]`: dispatch the `apply` trap or call the target.
    pub(in crate::runtime) fn proxy_apply(
        &mut self,
        id: ObjectId,
        args: &[Value],
        this_value: Value,
    ) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_APPLY)? else {
            return self.eval_call_value(target, args, this_value);
        };
        let args_array = self.create_array_from_elements(args.to_vec())?;
        self.eval_call_value(trap, &[target, this_value, args_array], handler)
    }

    /// Proxy `[[Construct]]`: dispatch the `construct` trap or construct the
    /// target. The proxy itself is passed as the new target.
    pub(in crate::runtime) fn proxy_construct(
        &mut self,
        id: ObjectId,
        args: &[Value],
    ) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.proxy_trap(&handler, PROXY_TRAP_CONSTRUCT)? else {
            return self.eval_new_value(target, args);
        };
        let args_array = self.create_array_from_elements(args.to_vec())?;
        let new_target = Value::Object(id);
        let result = self.eval_call_value(trap, &[target, args_array, new_target], handler)?;
        if !Self::is_object_like(&result) {
            return Err(Error::type_error(PROXY_CONSTRUCT_INVALID_ERROR));
        }
        Ok(result)
    }

    /// Enumerable own string keys of a proxy: the `ownKeys` trap result
    /// filtered by each key's `getOwnPropertyDescriptor` enumerability. Backs
    /// Object.keys/entries/values over a proxy.
    pub(in crate::runtime) fn proxy_enumerable_keys(&mut self, id: ObjectId) -> Result<Vec<String>> {
        let all = self.proxy_own_keys(id)?;
        let mut keys = Vec::new();
        for key in all {
            self.step()?;
            if let Some(descriptor) = self.proxy_get_own_property_descriptor(id, &key)?
                && Self::descriptor_is_enumerable(&descriptor)
            {
                keys.push(key);
            }
        }
        Ok(keys)
    }

    const fn descriptor_is_enumerable(descriptor: &OwnPropertyDescriptor) -> bool {
        matches!(
            descriptor,
            OwnPropertyDescriptor::Data(data) if data.enumerable().is_yes()
        ) || matches!(
            descriptor,
            OwnPropertyDescriptor::Accessor(accessor) if accessor.enumerable().is_yes()
        )
    }

    /// Convert the array-like result of an `ownKeys` trap into string keys.
    fn proxy_key_list_from_value(&mut self, value: &Value) -> Result<Vec<String>> {
        if !matches!(value, Value::Object(_)) {
            return Err(Error::type_error(
                "proxy ownKeys trap must return an array-like object",
            ));
        }
        let length_value = self.get_property_value(value, "length")?;
        let length = Self::reflect_length_from_value(&length_value)?;
        let mut keys = Vec::new();
        for index in 0..length {
            self.step()?;
            let element = self.get_property_value(value, &index.to_string())?;
            match element {
                Value::String(text) => keys.push(text),
                Value::HeapString(text) => keys.push(text.as_str().to_owned()),
                _ => {
                    return Err(Error::type_error(
                        "proxy ownKeys trap keys must be strings",
                    ));
                }
            }
        }
        Ok(keys)
    }
}
