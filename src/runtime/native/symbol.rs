use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{
        DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
        PropertyWritable,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, SYMBOL_NAME};

const SYMBOL_ASYNC_DISPOSE_PROPERTY: &str = "asyncDispose";
const SYMBOL_ASYNC_ITERATOR_PROPERTY: &str = "asyncIterator";
const SYMBOL_DESCRIPTION_PREFIX: &str = "Symbol.";
const SYMBOL_DISPOSE_PROPERTY: &str = "dispose";
const SYMBOL_HAS_INSTANCE_PROPERTY: &str = "hasInstance";
const SYMBOL_IS_CONCAT_SPREADABLE_PROPERTY: &str = "isConcatSpreadable";
const SYMBOL_ITERATOR_PROPERTY: &str = "iterator";
const SYMBOL_MATCH_ALL_PROPERTY: &str = "matchAll";
const SYMBOL_MATCH_PROPERTY: &str = "match";
const SYMBOL_METADATA_PROPERTY: &str = "metadata";
const SYMBOL_REPLACE_PROPERTY: &str = "replace";
const SYMBOL_SEARCH_PROPERTY: &str = "search";
const SYMBOL_SPECIES_PROPERTY: &str = "species";
const SYMBOL_SPLIT_PROPERTY: &str = "split";
const SYMBOL_TO_PRIMITIVE_PROPERTY: &str = "toPrimitive";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_UNSCOPABLES_PROPERTY: &str = "unscopables";

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
    pub(super) fn symbol_constructor_value(&mut self) -> Result<Value> {
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
        self.install_well_known_symbols(id)?;
        self.insert_global_builtin(SYMBOL_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_symbol_constructor(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let description = self.symbol_description(args)?;
        self.create_symbol_value(description.as_deref())
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

    fn install_well_known_symbols(&mut self, constructor: NativeFunctionId) -> Result<()> {
        for name in WELL_KNOWN_SYMBOL_PROPERTIES {
            let description = well_known_symbol_description(name)?;
            let value = self.create_symbol_value(Some(&description))?;
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

    fn symbol_description(&self, args: RuntimeCallArgs<'_>) -> Result<Option<String>> {
        let Some(value) = args.as_slice().first() else {
            return Ok(None);
        };
        match value {
            Value::Undefined => Ok(None),
            Value::Symbol(_) => Err(Error::runtime("Cannot convert a Symbol value to a string")),
            Value::String(value) => {
                self.check_string_len(value)?;
                Ok(Some(value.clone()))
            }
            Value::HeapString(value) => {
                self.check_string_len(value.as_str())?;
                Ok(Some(value.as_str().to_owned()))
            }
            value => {
                let description = value.to_string();
                self.check_string_len(&description)?;
                Ok(Some(description))
            }
        }
    }
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
