use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        native::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, TypedArrayFunctionKind},
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    value::{NativeFunctionId, ObjectId, Value},
};

const FROM_PROPERTY: &str = "from";
const OF_PROPERTY: &str = "of";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const TO_STRING_TAG_SYMBOL_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_SYMBOL_PROPERTY: &str = "toStringTag";

const ACCESSORS: &[(&str, TypedArrayFunctionKind)] = &[
    ("buffer", TypedArrayFunctionKind::BufferGetter),
    ("byteLength", TypedArrayFunctionKind::ByteLengthGetter),
    ("byteOffset", TypedArrayFunctionKind::ByteOffsetGetter),
    ("length", TypedArrayFunctionKind::LengthGetter),
];

const METHODS: &[(&str, TypedArrayFunctionKind)] = &[
    ("at", TypedArrayFunctionKind::At),
    ("copyWithin", TypedArrayFunctionKind::CopyWithin),
    ("entries", TypedArrayFunctionKind::Entries),
    ("every", TypedArrayFunctionKind::Every),
    ("fill", TypedArrayFunctionKind::Fill),
    ("filter", TypedArrayFunctionKind::Filter),
    ("find", TypedArrayFunctionKind::Find),
    ("findIndex", TypedArrayFunctionKind::FindIndex),
    ("findLast", TypedArrayFunctionKind::FindLast),
    ("findLastIndex", TypedArrayFunctionKind::FindLastIndex),
    ("forEach", TypedArrayFunctionKind::ForEach),
    ("includes", TypedArrayFunctionKind::Includes),
    ("indexOf", TypedArrayFunctionKind::IndexOf),
    ("join", TypedArrayFunctionKind::Join),
    ("keys", TypedArrayFunctionKind::Keys),
    ("lastIndexOf", TypedArrayFunctionKind::LastIndexOf),
    ("map", TypedArrayFunctionKind::Map),
    ("reduce", TypedArrayFunctionKind::Reduce),
    ("reduceRight", TypedArrayFunctionKind::ReduceRight),
    ("reverse", TypedArrayFunctionKind::Reverse),
    ("set", TypedArrayFunctionKind::Set),
    ("slice", TypedArrayFunctionKind::Slice),
    ("some", TypedArrayFunctionKind::Some),
    ("sort", TypedArrayFunctionKind::Sort),
    ("subarray", TypedArrayFunctionKind::Subarray),
    ("toLocaleString", TypedArrayFunctionKind::ToLocaleString),
    ("toReversed", TypedArrayFunctionKind::ToReversed),
    ("toSorted", TypedArrayFunctionKind::ToSorted),
    ("toString", TypedArrayFunctionKind::ToString),
    ("with", TypedArrayFunctionKind::With),
];

impl Context {
    pub(in crate::runtime) fn typed_array_intrinsic_constructor_value(&mut self) -> Result<Value> {
        let kind = NativeFunctionKind::TypedArrayIntrinsic;
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
        self.install_typed_array_intrinsic_statics(id)?;
        self.install_typed_array_prototype(prototype)?;
        Ok(constructor)
    }

    pub(super) fn typed_array_intrinsic_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.typed_array_intrinsic_constructor_value()? else {
            return Err(Error::runtime("%TypedArray% constructor is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(prototype) => Ok(prototype),
            _ => Err(Error::runtime("%TypedArray%.prototype is not an object")),
        }
    }

    fn install_typed_array_intrinsic_statics(&mut self, id: NativeFunctionId) -> Result<()> {
        for (name, kind) in [
            (FROM_PROPERTY, TypedArrayFunctionKind::From),
            (OF_PROPERTY, TypedArrayFunctionKind::Of),
        ] {
            let method = self.create_native_function(
                NativeFunctionKind::TypedArrayPrototype(kind),
                Value::Undefined,
            )?;
            let key = self.intern_property_key(name)?;
            self.define_native_function_property_key(
                id,
                name,
                key,
                DataPropertyUpdate::new(
                    Some(method),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ),
            )?;
        }
        self.install_species_accessor(id)
    }

    fn install_typed_array_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in ACCESSORS {
            self.define_typed_array_accessor(prototype, name, *kind)?;
        }
        for (name, kind) in METHODS {
            self.define_typed_array_method(prototype, name, *kind)?;
        }
        let values = self.create_native_function(
            NativeFunctionKind::TypedArrayPrototype(TypedArrayFunctionKind::Values),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, "values", values.clone())?;
        self.install_typed_array_iterator_alias(prototype, values)?;
        self.install_typed_array_to_string_tag(prototype)
    }

    fn define_typed_array_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: TypedArrayFunctionKind,
    ) -> Result<()> {
        let getter = self.create_native_function(
            NativeFunctionKind::TypedArrayPrototype(kind),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            prototype,
            key,
            name,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn define_typed_array_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: TypedArrayFunctionKind,
    ) -> Result<()> {
        let method = self.create_native_function(
            NativeFunctionKind::TypedArrayPrototype(kind),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, name, method)
    }

    fn install_typed_array_iterator_alias(
        &mut self,
        prototype: ObjectId,
        values: Value,
    ) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(values),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_typed_array_to_string_tag(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag = self.get_named(&symbol_constructor, TO_STRING_TAG_SYMBOL_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let getter = self.create_native_function(
            NativeFunctionKind::TypedArrayPrototype(TypedArrayFunctionKind::ToStringTagGetter),
            Value::Undefined,
        )?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
