use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::to_boolean, call::RuntimeCallArgs,
        object::OwnPropertyDescriptor, object::PropertyEnumerable, object::PropertyKey,
        object::PropertyLookup, object::PropertyUpdate, roots::VmRootKind,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{
    NativeFunctionKind, PROXY_REVOCABLE_NAME,
    proxy_invariants::{
        validate_define_property, validate_delete, validate_get,
        validate_get_own_property_descriptor, validate_get_prototype, validate_has,
        validate_is_extensible, validate_prevent_extensions, validate_set, validate_set_prototype,
    },
};

const PROXY_REVOCABLE_PROXY_PROPERTY: &str = "proxy";
const PROXY_REVOCABLE_REVOKE_PROPERTY: &str = "revoke";

const PROXY_TARGET_NOT_OBJECT_ERROR: &str = "Proxy target must be an object";
const PROXY_HANDLER_NOT_OBJECT_ERROR: &str = "Proxy handler must be an object";
const PROXY_REQUIRES_NEW_ERROR: &str = "Constructor Proxy requires 'new'";
const PROXY_REVOKED_ERROR: &str = "Cannot perform operation on a revoked Proxy";
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
        let constructor =
            self.create_native_function(NativeFunctionKind::Proxy, Value::Undefined)?;
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
            .define_builtin(key, function, PropertyEnumerable::No)?;
        Ok(())
    }

    /// Spec `Proxy(target, handler)` / `ProxyCreate`. Both operands must be
    /// objects (ordinary objects or callables). Calling `Proxy` without `new`
    /// is a `TypeError`.
    pub(in crate::runtime) fn eval_proxy_call(_args: RuntimeCallArgs<'_>) -> Result<Value> {
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
        if self.semantic_object_ref(&target)?.is_none() {
            return Err(Error::type_error(PROXY_TARGET_NOT_OBJECT_ERROR));
        }
        if self.semantic_object_ref(&handler)?.is_none() {
            return Err(Error::type_error(PROXY_HANDLER_NOT_OBJECT_ERROR));
        }
        let callable = self.semantic_is_callable(&target)?;
        let constructable = self.semantic_is_constructor(&target)?;
        self.objects.create_proxy_object(
            target,
            handler,
            callable,
            constructable,
            self.limits.max_objects,
        )
    }

    /// Resolve the wrapped target and handler for a proxy object, raising a
    /// `TypeError` when the proxy has been revoked.
    pub(in crate::runtime) fn proxy_target_handler(&self, id: ObjectId) -> Result<(Value, Value)> {
        let proxy = self
            .objects
            .proxy_value(id)?
            .ok_or_else(|| Error::runtime("object is not a proxy"))?;
        let (Some(target), Some(handler)) = (proxy.target(), proxy.handler()) else {
            return Err(Error::type_error(PROXY_REVOKED_ERROR));
        };
        Ok((target.clone(), handler.clone()))
    }

    /// Proxy `[[Get]]`: dispatch to the `get` trap or fall back to the target.
    pub(in crate::runtime) fn proxy_get(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
        receiver: Value,
    ) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_GET)? else {
            let Some(read) =
                self.semantic_property_read_with_receiver(&target, &receiver, property)?
            else {
                return Err(Error::type_error("proxy target is not an object"));
            };
            return self.finish_semantic_property_read(read, &receiver, property);
        };
        let key = self.proxy_property_key_value(property)?;
        let result = self
            .call(&trap, &[target.clone(), key, receiver], handler)?
            .into_native_value_result()?;
        let descriptor = self.proxy_target_descriptor(&target, property)?;
        validate_get(descriptor.as_ref(), &result)?;
        Ok(result)
    }

    /// Proxy `[[Has]]`: dispatch to the `has` trap or fall back to the target.
    pub(in crate::runtime) fn proxy_has(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_HAS)? else {
            return self.has_property_value_with_lookup(&target, property);
        };
        let key = self.proxy_property_key_value(property)?;
        let result = self
            .call(&trap, &[target.clone(), key], handler)?
            .into_native_value_result()?;
        let present = to_boolean(&result);
        if !present {
            let descriptor = self.proxy_target_descriptor(&target, property)?;
            let extensible = self.proxy_target_is_extensible(&target)?;
            validate_has(descriptor.as_ref(), extensible)?;
        }
        Ok(present)
    }

    fn proxy_property_key_value(&mut self, property: PropertyLookup<'_>) -> Result<Value> {
        if let Some(symbol) = property.key().and_then(PropertyKey::symbol_id) {
            return self.symbols.get(symbol).cloned().map(Value::Symbol);
        }
        self.heap_string_value(property.name())
    }

    fn proxy_target_descriptor(
        &mut self,
        target: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let key = self.proxy_property_key_value(property)?;
        let dynamic = self.dynamic_property_key(&key)?;
        self.semantic_own_property_descriptor(target, &dynamic)
    }

    fn proxy_target_is_extensible(&mut self, target: &Value) -> Result<bool> {
        self.semantic_is_extensible(target)?
            .ok_or_else(|| Error::type_error("proxy target is not an object"))
    }

    /// Proxy `[[Set]]`: dispatch to the `set` trap or fall back to the target.
    /// Returns whether the assignment succeeded.
    pub(in crate::runtime) fn proxy_set(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
        value: Value,
        receiver: Value,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_SET)? else {
            return self.set(
                &target,
                property,
                value,
                &receiver,
                crate::runtime::abstract_operations::SetFailureBehavior::ReturnFalse,
            );
        };
        let key = self.proxy_property_key_value(property)?;
        let result = self
            .call(
                &trap,
                &[target.clone(), key, value.clone(), receiver],
                handler,
            )?
            .into_native_value_result()?;
        let updated = to_boolean(&result);
        if updated {
            let descriptor = self.proxy_target_descriptor(&target, property)?;
            validate_set(descriptor.as_ref(), &value)?;
        }
        Ok(updated)
    }

    /// Proxy `[[Delete]]`: dispatch to the `deleteProperty` trap or fall back to
    /// the target. Returns whether the deletion succeeded.
    pub(in crate::runtime) fn proxy_delete(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_DELETE)? else {
            return self.delete_property_value_with_lookup(&target, property);
        };
        let key = self.proxy_property_key_value(property)?;
        let result = self
            .call(&trap, &[target.clone(), key], handler)?
            .into_native_value_result()?;
        let deleted = to_boolean(&result);
        if deleted {
            let descriptor = self.proxy_target_descriptor(&target, property)?;
            let extensible = self.proxy_target_is_extensible(&target)?;
            validate_delete(descriptor.as_ref(), extensible)?;
        }
        Ok(deleted)
    }

    /// Proxy `[[GetPrototypeOf]]`.
    pub(in crate::runtime) fn proxy_get_prototype_of(&mut self, id: ObjectId) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_GET_PROTOTYPE_OF)? else {
            return self
                .semantic_get_prototype(&target)?
                .ok_or_else(|| Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR));
        };
        let result = self
            .call(&trap, std::slice::from_ref(&target), handler)?
            .into_native_value_result()?;
        if !matches!(result, Value::Object(_) | Value::Null) {
            return Err(Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR));
        }
        let extensible = self.proxy_target_is_extensible(&target)?;
        if !extensible {
            let target_prototype = self
                .semantic_get_prototype(&target)?
                .ok_or_else(|| Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR))?;
            validate_get_prototype(extensible, &target_prototype, &result)?;
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
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_SET_PROTOTYPE_OF)? else {
            return self
                .semantic_try_set_prototype(&target, prototype)?
                .ok_or_else(|| Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR));
        };
        let result = self
            .call(&trap, &[target.clone(), prototype.clone()], handler)?
            .into_native_value_result()?;
        let updated = to_boolean(&result);
        if !updated {
            return Ok(false);
        }
        let extensible = self.proxy_target_is_extensible(&target)?;
        if !extensible {
            let target_prototype = self
                .semantic_get_prototype(&target)?
                .ok_or_else(|| Error::type_error(PROXY_GET_PROTOTYPE_INVALID_ERROR))?;
            validate_set_prototype(extensible, &target_prototype, &prototype)?;
        }
        Ok(true)
    }

    /// Proxy `[[IsExtensible]]`.
    pub(in crate::runtime) fn proxy_is_extensible(&mut self, id: ObjectId) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_IS_EXTENSIBLE)? else {
            return self
                .semantic_is_extensible(&target)?
                .ok_or_else(|| Error::type_error("proxy target is not an object"));
        };
        let result = self
            .call(&trap, std::slice::from_ref(&target), handler)?
            .into_native_value_result()?;
        let extensible = to_boolean(&result);
        let target_extensible = self.proxy_target_is_extensible(&target)?;
        validate_is_extensible(target_extensible, extensible)?;
        Ok(extensible)
    }

    /// Proxy `[[PreventExtensions]]`.
    pub(in crate::runtime) fn proxy_prevent_extensions(&mut self, id: ObjectId) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_PREVENT_EXTENSIONS)? else {
            return self
                .semantic_prevent_extensions(&target)?
                .ok_or_else(|| Error::type_error("proxy target is not an object"));
        };
        let result = self
            .call(&trap, std::slice::from_ref(&target), handler)?
            .into_native_value_result()?;
        let prevented = to_boolean(&result);
        if prevented {
            validate_prevent_extensions(self.proxy_target_is_extensible(&target)?)?;
        }
        Ok(prevented)
    }

    /// Proxy `[[DefineOwnProperty]]`: dispatch the `defineProperty` trap or
    /// define the property on the target. Returns whether the definition
    /// succeeded.
    pub(in crate::runtime) fn proxy_define_property(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
        update: PropertyUpdate,
        descriptor: Value,
    ) -> Result<bool> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let key = self.proxy_property_key_value(property)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_DEFINE_PROPERTY)? else {
            let mut dynamic = self.dynamic_property_key(&key)?;
            return self.semantic_define_own_property_update_with_descriptor(
                &target,
                &mut dynamic,
                update,
                &descriptor,
            );
        };
        let result = self
            .call(&trap, &[target.clone(), key, descriptor], handler)?
            .into_native_value_result()?;
        let defined = to_boolean(&result);
        if !defined {
            return Ok(false);
        }
        let target_descriptor = self.proxy_target_descriptor(&target, property)?;
        let extensible = self.proxy_target_is_extensible(&target)?;
        validate_define_property(&update, target_descriptor.as_ref(), extensible)?;
        Ok(true)
    }

    /// Proxy `[[OwnPropertyKeys]]`: dispatch the `ownKeys` trap or read the
    /// target's own keys while preserving string and Symbol values.
    pub(in crate::runtime) fn proxy_own_property_keys(
        &mut self,
        id: ObjectId,
    ) -> Result<Vec<Value>> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_OWN_KEYS)? else {
            return self.semantic_own_property_keys(&target);
        };
        let result = self
            .call(&trap, std::slice::from_ref(&target), handler)?
            .into_native_value_result()?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let keys = self.proxy_key_list_from_value(&result)?;
        self.validate_proxy_own_property_keys(&target, &keys)?;
        Ok(keys)
    }

    /// Proxy `[[GetOwnProperty]]`: dispatch the `getOwnPropertyDescriptor` trap
    /// or read the target's own descriptor.
    pub(in crate::runtime) fn proxy_get_own_property_descriptor(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let key_value = self.proxy_property_key_value(property)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_GET_OWN_DESCRIPTOR)? else {
            let dynamic = self.dynamic_property_key(&key_value)?;
            return self.semantic_own_property_descriptor(&target, &dynamic);
        };
        let result = self
            .call(&trap, &[target.clone(), key_value], handler)?
            .into_native_value_result()?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let descriptor = match result {
            Value::Undefined => None,
            Value::Object(_) => Some(self.own_property_descriptor_from_object(&result)?),
            _ => return Err(Error::type_error(PROXY_DESCRIPTOR_INVALID_ERROR)),
        };
        let target_descriptor = self.proxy_target_descriptor(&target, property)?;
        let extensible = self.proxy_target_is_extensible(&target)?;
        validate_get_own_property_descriptor(
            descriptor.as_ref(),
            target_descriptor.as_ref(),
            extensible,
        )?;
        Ok(descriptor)
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
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_APPLY)? else {
            return self.call_value(&target, args, this_value);
        };
        let args_array = self.create_array_from_elements(args.to_vec())?;
        self.call(&trap, &[target, this_value, args_array], handler)?
            .into_native_value_result()
    }

    /// Proxy `[[Construct]]`: dispatch the `construct` trap or construct the
    /// target while preserving the caller's explicit `newTarget`.
    pub(in crate::runtime) fn proxy_construct(
        &mut self,
        id: ObjectId,
        args: &[Value],
        new_target: Value,
    ) -> Result<Value> {
        let (target, handler) = self.proxy_target_handler(id)?;
        let Some(trap) = self.get_named_method(&handler, PROXY_TRAP_CONSTRUCT)? else {
            return self.semantic_construct(&target, args, new_target);
        };
        let args_array = self.create_array_from_elements(args.to_vec())?;
        let result = self
            .call(&trap, &[target, args_array, new_target], handler)?
            .into_native_value_result()?;
        if self.semantic_object_ref(&result)?.is_none() {
            return Err(Error::type_error(PROXY_CONSTRUCT_INVALID_ERROR));
        }
        Ok(result)
    }

    fn validate_proxy_own_property_keys(
        &mut self,
        target: &Value,
        trap_keys: &[Value],
    ) -> Result<()> {
        let target_keys = self.semantic_own_property_keys(target)?;
        let extensible = self
            .semantic_is_extensible(target)?
            .ok_or_else(|| Error::type_error("proxy ownKeys target is not an object"))?;
        for target_key in &target_keys {
            self.step()?;
            let property = self.dynamic_property_key(target_key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(target, &property)? else {
                continue;
            };
            if !Self::descriptor_is_configurable(&descriptor) && !trap_keys.contains(target_key) {
                return Err(Error::type_error(
                    "proxy ownKeys trap omitted a non-configurable target key",
                ));
            }
        }
        if extensible {
            return Ok(());
        }
        if target_keys.len() != trap_keys.len()
            || target_keys.iter().any(|key| !trap_keys.contains(key))
        {
            return Err(Error::type_error(
                "proxy ownKeys trap keys differ from a non-extensible target",
            ));
        }
        Ok(())
    }

    const fn descriptor_is_configurable(descriptor: &OwnPropertyDescriptor) -> bool {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => descriptor.configurable().is_yes(),
            OwnPropertyDescriptor::Accessor(descriptor) => descriptor.configurable().is_yes(),
        }
    }

    /// Convert the array-like result of an `ownKeys` trap into string keys.
    fn proxy_key_list_from_value(&mut self, value: &Value) -> Result<Vec<Value>> {
        if self.semantic_object_ref(value)?.is_none() {
            return Err(Error::type_error(
                "proxy ownKeys trap must return an array-like object",
            ));
        }
        let length_value = self.get_named(value, "length")?;
        let length = self.reflect_length_from_value(&length_value)?;
        let mut keys = Vec::new();
        for index in 0..length {
            self.step()?;
            let element = self.get_named(value, &index.to_string())?;
            let key = match element {
                Value::String(_) | Value::HeapString(_) | Value::Symbol(_) => element,
                _ => {
                    return Err(Error::type_error(
                        "proxy ownKeys trap keys must be strings or symbols",
                    ));
                }
            };
            if keys.contains(&key) {
                return Err(Error::type_error(
                    "proxy ownKeys trap returned a duplicate key",
                ));
            }
            keys.push(key);
        }
        Ok(keys)
    }
}
