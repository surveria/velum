use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::same_value,
        call::RuntimeCallArgs,
        collections::CollectionKind,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ObjectId, Value},
};

use super::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

pub(in crate::runtime::native) const FINALIZATION_REGISTRY_NAME: &str = "FinalizationRegistry";
const REGISTER_NAME: &str = "register";
const UNREGISTER_NAME: &str = "unregister";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const CLEANUP_CALLBACK_ERROR: &str = "FinalizationRegistry cleanup callback must be callable";
const TARGET_ERROR: &str = "FinalizationRegistry target must be held weakly";
const HELD_VALUE_ERROR: &str = "FinalizationRegistry target and held value must differ";
const UNREGISTER_TOKEN_ERROR: &str = "FinalizationRegistry unregister token must be held weakly";

impl Context {
    pub(in crate::runtime::native) fn finalization_registry_constructor_value(
        &mut self,
    ) -> Result<Value> {
        let kind = NativeFunctionKind::FinalizationRegistry;
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor.clone(),
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        self.install_finalization_registry_prototype(prototype)?;
        self.insert_global_builtin(FINALIZATION_REGISTRY_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn construct_finalization_registry(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let callback = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(CLEANUP_CALLBACK_ERROR));
        }
        let prototype = self.finalization_registry_intrinsic_prototype()?;
        self.create_finalization_registry_with_prototype(prototype, callback)
    }

    fn finalization_registry_intrinsic_prototype(&mut self) -> Result<ObjectId> {
        let constructor = self.finalization_registry_constructor_value()?;
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime(
                "FinalizationRegistry constructor disappeared",
            ));
        };
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime(
                "FinalizationRegistry prototype is not an object",
            ));
        };
        Ok(prototype)
    }

    fn create_finalization_registry_with_prototype(
        &mut self,
        prototype: ObjectId,
        callback: Value,
    ) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = value else {
            return Err(Error::runtime("FinalizationRegistry allocation failed"));
        };
        let collection = self.create_collection(CollectionKind::FinalizationRegistry)?;
        self.initialize_finalization_registry(collection, callback)?;
        self.bind_collection_object(object, CollectionKind::FinalizationRegistry, collection)?;
        Ok(Value::Object(object))
    }

    fn install_finalization_registry_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                REGISTER_NAME,
                NativeFunctionKind::FinalizationRegistryRegister,
            ),
            (
                UNREGISTER_NAME,
                NativeFunctionKind::FinalizationRegistryUnregister,
            ),
        ] {
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag = self.heap_string_value(FINALIZATION_REGISTRY_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_finalization_registry_register(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let registry =
            self.collection_from_this(this_value, CollectionKind::FinalizationRegistry)?;
        let target = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.can_be_held_weakly(&target)? {
            return Err(Error::type_error(TARGET_ERROR));
        }
        let held_value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if same_value(&target, &held_value) {
            return Err(Error::type_error(HELD_VALUE_ERROR));
        }
        let token = args.as_slice().get(2).cloned().unwrap_or(Value::Undefined);
        let unregister_token = if matches!(token, Value::Undefined) {
            None
        } else {
            if !self.can_be_held_weakly(&token)? {
                return Err(Error::type_error(UNREGISTER_TOKEN_ERROR));
            }
            Some(token)
        };
        self.register_finalization(registry, target, held_value, unregister_token)?;
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_finalization_registry_unregister(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let registry =
            self.collection_from_this(this_value, CollectionKind::FinalizationRegistry)?;
        let token = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.can_be_held_weakly(&token)? {
            return Err(Error::type_error(UNREGISTER_TOKEN_ERROR));
        }
        self.unregister_finalizations(registry, &token)
            .map(Value::Bool)
    }
}
