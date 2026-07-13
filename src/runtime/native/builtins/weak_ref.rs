use crate::{
    error::{Error, Result},
    runtime::{
        Context,
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

pub(in crate::runtime::native) const WEAK_REF_NAME: &str = "WeakRef";
const DEREF_NAME: &str = "deref";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const TARGET_ERROR: &str = "WeakRef target must be held weakly";

impl Context {
    pub(in crate::runtime::native) fn weak_ref_constructor_value(&mut self) -> Result<Value> {
        let kind = NativeFunctionKind::WeakRef;
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
        self.install_weak_ref_prototype(prototype)?;
        self.insert_global_builtin(WEAK_REF_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn construct_weak_ref(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let target = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.can_be_held_weakly(&target)? {
            return Err(Error::type_error(TARGET_ERROR));
        }
        let prototype = self.weak_ref_intrinsic_prototype()?;
        self.create_weak_ref_with_prototype(prototype, target)
    }

    fn weak_ref_intrinsic_prototype(&mut self) -> Result<ObjectId> {
        let constructor = self.weak_ref_constructor_value()?;
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime("WeakRef constructor disappeared"));
        };
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime("WeakRef prototype is not an object"));
        };
        Ok(prototype)
    }

    fn create_weak_ref_with_prototype(
        &mut self,
        prototype: ObjectId,
        target: Value,
    ) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = value else {
            return Err(Error::runtime("WeakRef allocation failed"));
        };
        let collection = self.create_collection(CollectionKind::WeakRef)?;
        self.initialize_weak_ref(collection, target)?;
        self.bind_collection_object(object, CollectionKind::WeakRef, collection)?;
        Ok(Value::Object(object))
    }

    fn install_weak_ref_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        let deref = self
            .create_ephemeral_native_function(NativeFunctionKind::WeakRefDeref, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, DEREF_NAME, deref)?;
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag = self.heap_string_value(WEAK_REF_NAME)?;
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

    pub(in crate::runtime::native) fn eval_weak_ref_deref(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        let weak_ref = self.collection_from_this(this_value, CollectionKind::WeakRef)?;
        self.weak_ref_target(weak_ref)
    }
}
