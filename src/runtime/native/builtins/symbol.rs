use crate::runtime::native::function::SYMBOL_PROTOTYPE_TO_PRIMITIVE_NAME;
use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, ObjectPrimitiveValue, ObjectPropertyInit,
        PropertyConfigurable, PropertyEnumerable, PropertyLookup, PropertyUpdate, PropertyWritable,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, SYMBOL_NAME, SYMBOL_PROTOTYPE_TO_STRING_NAME,
    SYMBOL_PROTOTYPE_VALUE_OF_NAME,
};

const SYMBOL_ASYNC_DISPOSE_PROPERTY: &str = "asyncDispose";
const SYMBOL_ASYNC_ITERATOR_PROPERTY: &str = "asyncIterator";
const SYMBOL_DESCRIPTION_PREFIX: &str = "Symbol.";
const SYMBOL_DISPOSE_PROPERTY: &str = "dispose";
const SYMBOL_FOR_PROPERTY: &str = "for";
const SYMBOL_HAS_INSTANCE_PROPERTY: &str = "hasInstance";
const SYMBOL_IS_CONCAT_SPREADABLE_PROPERTY: &str = "isConcatSpreadable";
const SYMBOL_ITERATOR_PROPERTY: &str = "iterator";
const SYMBOL_KEY_FOR_PROPERTY: &str = "keyFor";
const SYMBOL_MATCH_ALL_PROPERTY: &str = "matchAll";
const SYMBOL_MATCH_PROPERTY: &str = "match";
const SYMBOL_METADATA_PROPERTY: &str = "metadata";
const SYMBOL_PROTOTYPE_DESCRIPTION_PROPERTY: &str = "description";
const SYMBOL_REPLACE_PROPERTY: &str = "replace";
const SYMBOL_SEARCH_PROPERTY: &str = "search";
const SYMBOL_SPECIES_PROPERTY: &str = "species";
const SYMBOL_SPLIT_PROPERTY: &str = "split";
const SYMBOL_TO_PRIMITIVE_PROPERTY: &str = "toPrimitive";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_UNSCOPABLES_PROPERTY: &str = "unscopables";
const SYMBOL_VALUE_RECEIVER_ERROR: &str =
    "Symbol.prototype value method requires a symbol or Symbol object";

const WELL_KNOWN_SYMBOL_PROPERTIES: &[&str] = &[
    SYMBOL_ASYNC_DISPOSE_PROPERTY,
    SYMBOL_ASYNC_ITERATOR_PROPERTY,
    SYMBOL_DISPOSE_PROPERTY,
    SYMBOL_HAS_INSTANCE_PROPERTY,
    SYMBOL_IS_CONCAT_SPREADABLE_PROPERTY,
    SYMBOL_ITERATOR_PROPERTY,
    SYMBOL_MATCH_PROPERTY,
    SYMBOL_MATCH_ALL_PROPERTY,
    SYMBOL_METADATA_PROPERTY,
    SYMBOL_REPLACE_PROPERTY,
    SYMBOL_SEARCH_PROPERTY,
    SYMBOL_SPECIES_PROPERTY,
    SYMBOL_SPLIT_PROPERTY,
    SYMBOL_TO_PRIMITIVE_PROPERTY,
    SYMBOL_TO_STRING_TAG_PROPERTY,
    SYMBOL_UNSCOPABLES_PROPERTY,
];

impl Context {
    pub(in crate::runtime) fn symbol_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Symbol) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.symbol_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::Symbol)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Symbol, prototype, name)?;
        self.install_symbol_static_methods(id)?;
        self.install_well_known_symbols(id)?;
        self.install_symbol_prototype_methods(prototype_id, id)?;
        self.insert_global_builtin(SYMBOL_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_symbol_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let description = self.symbol_description(args)?;
        self.create_symbol_value(description.as_deref())
    }

    pub(in crate::runtime::native) fn eval_symbol_for(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let key = self.symbol_registry_key(args.as_slice().first())?;
        let adds_association = !self.symbols.has_registry_key(&key);
        if adds_association {
            self.storage_ledger
                .grow_count(crate::runtime::VmStorageKind::Association, 1)?;
        }
        match self.symbols.for_key(key) {
            Ok(symbol) => Ok(Value::Symbol(symbol)),
            Err(error) => {
                if adds_association {
                    self.storage_ledger
                        .release_count(crate::runtime::VmStorageKind::Association, 1)?;
                }
                Err(error)
            }
        }
    }

    pub(in crate::runtime::native) fn eval_symbol_key_for(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(Value::Symbol(symbol)) = args.as_slice().first() else {
            return Err(Error::type_error("Symbol.keyFor requires a symbol value"));
        };
        let Some(key) = self.symbols.key_for(symbol.id())? else {
            return Ok(Value::Undefined);
        };
        Ok(Value::HeapString(key))
    }

    pub(in crate::runtime::native) fn create_symbol_object_from_value(
        &mut self,
        value: crate::storage::symbol::JsSymbol,
    ) -> Result<Value> {
        let prototype = self.symbol_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Symbol(value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime::native) fn eval_symbol_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_symbol_extra_args(args.as_slice());
        self.eval_direct_symbol_prototype_to_string(this_value)
    }

    pub(in crate::runtime) fn eval_direct_symbol_prototype_to_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let symbol = self.symbol_receiver_value(this_value)?;
        self.heap_string_value(&symbol.display_name())
    }

    pub(in crate::runtime::native) fn eval_symbol_prototype_value_of(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_symbol_extra_args(args.as_slice());
        self.eval_direct_symbol_prototype_value_of(this_value)
    }

    pub(in crate::runtime::native) fn eval_symbol_prototype_to_primitive(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_symbol_extra_args(args.as_slice());
        self.eval_direct_symbol_prototype_value_of(this_value)
    }

    pub(in crate::runtime) fn eval_direct_symbol_prototype_value_of(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.symbol_receiver_value(this_value).map(Value::Symbol)
    }

    pub(in crate::runtime::native) fn eval_symbol_prototype_description(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_symbol_extra_args(args.as_slice());
        self.eval_direct_symbol_prototype_description(this_value)
    }

    pub(in crate::runtime) fn eval_direct_symbol_prototype_description(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let symbol = self.symbol_receiver_value(this_value)?;
        let Some(description) = symbol.description() else {
            return Ok(Value::Undefined);
        };
        self.heap_string_value(description)
    }

    pub(in crate::runtime) fn symbol_prototype_property_value(
        &mut self,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.symbol_constructor_prototype()?;
        self.get_prototype_property_value_with_receiver(prototype, receiver, property)
    }

    pub(in crate::runtime) fn symbol_prototype_property_value_with_lookup(
        &mut self,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let prototype = self.symbol_constructor_prototype()?;
        self.get_prototype_property_value_with_lookup(prototype, receiver, property)
    }

    fn symbol_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn symbol_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.symbol_constructor_value()? else {
            return Err(Error::runtime("Symbol constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Symbol prototype is not an object")),
        }
    }

    fn install_well_known_symbols(&mut self, constructor: NativeFunctionId) -> Result<()> {
        for name in WELL_KNOWN_SYMBOL_PROPERTIES {
            let description = well_known_symbol_description(name)?;
            let value = self.create_symbol_value(Some(&description))?;
            if *name == SYMBOL_ITERATOR_PROPERTY
                && let Value::Symbol(symbol) = &value
            {
                self.set_iterator_symbol(symbol.id())?;
            }
            let key = self.intern_property_key(name)?;
            self.define_native_function_property_key(
                constructor,
                name,
                key,
                DataPropertyUpdate::new(
                    Some(value),
                    Some(PropertyWritable::No),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::No),
                ),
            )?;
        }
        Ok(())
    }

    fn install_symbol_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        self.define_symbol_static_method(
            constructor,
            SYMBOL_FOR_PROPERTY,
            NativeFunctionKind::SymbolFor,
        )?;
        self.define_symbol_static_method(
            constructor,
            SYMBOL_KEY_FOR_PROPERTY,
            NativeFunctionKind::SymbolKeyFor,
        )
    }

    fn define_symbol_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.define_native_function_property_key(
            constructor,
            name,
            key,
            DataPropertyUpdate::new(
                Some(function),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }

    fn install_symbol_prototype_methods(
        &mut self,
        prototype: ObjectId,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        self.define_symbol_prototype_accessor(
            prototype,
            SYMBOL_PROTOTYPE_DESCRIPTION_PROPERTY,
            NativeFunctionKind::SymbolPrototypeDescriptionGetter,
        )?;
        self.define_symbol_prototype_method(
            prototype,
            SYMBOL_PROTOTYPE_TO_STRING_NAME,
            NativeFunctionKind::SymbolPrototypeToString,
        )?;
        self.define_symbol_prototype_to_primitive(prototype, constructor)?;
        self.define_symbol_prototype_method(
            prototype,
            SYMBOL_PROTOTYPE_VALUE_OF_NAME,
            NativeFunctionKind::SymbolPrototypeValueOf,
        )
    }

    fn define_symbol_prototype_to_primitive(
        &mut self,
        prototype: ObjectId,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let symbol = self.get_named(
            &Value::NativeFunction(constructor),
            SYMBOL_TO_PRIMITIVE_PROPERTY,
        )?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.toPrimitive is not a symbol"));
        };
        let function = self.create_native_function(
            NativeFunctionKind::SymbolPrototypeToPrimitive,
            Value::Undefined,
        )?;
        self.objects.define_property(
            prototype,
            crate::runtime::object::PropertyKey::symbol(symbol.id()),
            SYMBOL_PROTOTYPE_TO_PRIMITIVE_NAME,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(function),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn define_symbol_prototype_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, function)
    }

    fn define_symbol_prototype_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let getter = self.create_native_function(kind, Value::Undefined)?;
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

    fn symbol_description(&mut self, args: RuntimeCallArgs<'_>) -> Result<Option<String>> {
        let Some(value) = args.as_slice().first() else {
            return Ok(None);
        };
        match value {
            Value::Undefined => Ok(None),
            Value::Symbol(_) => Err(Error::runtime("Cannot convert a Symbol value to a string")),
            value => self.to_string(value).map(Some),
        }
    }

    fn symbol_registry_key(
        &mut self,
        value: Option<&Value>,
    ) -> Result<crate::storage::string_heap::JsString> {
        let text = match value {
            Some(value) => self.to_string(value)?,
            None => self.to_string(&Value::Undefined)?,
        };
        self.intern_owned_heap_string(text)
    }

    fn symbol_receiver_value(&self, value: &Value) -> Result<crate::storage::symbol::JsSymbol> {
        match value {
            Value::Symbol(value) => Ok(value.clone()),
            Value::Object(id) => match self.objects.primitive_value(*id)? {
                Some(ObjectPrimitiveValue::Symbol(value)) => Ok(value.clone()),
                Some(ObjectPrimitiveValue::Bool(_) | ObjectPrimitiveValue::Number(_)) | None => {
                    Err(Error::type_error(SYMBOL_VALUE_RECEIVER_ERROR))
                }
            },
            _ => Err(Error::type_error(SYMBOL_VALUE_RECEIVER_ERROR)),
        }
    }

    const fn discard_symbol_extra_args(_args: &[Value]) {}
}

fn well_known_symbol_description(name: &str) -> Result<String> {
    let length = SYMBOL_DESCRIPTION_PREFIX
        .len()
        .checked_add(name.len())
        .ok_or_else(|| Error::limit("well-known symbol description length overflowed"))?;
    let mut description = String::with_capacity(length);
    description.push_str(SYMBOL_DESCRIPTION_PREFIX);
    description.push_str(name);
    Ok(description)
}
