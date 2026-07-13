use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::{ObjectId, Value},
};

use super::NativeFunctionKind;

const ARRAY_PROTOTYPE_CONCAT_PROPERTY: &str = "concat";
const ARRAY_PROTOTYPE_EVERY_PROPERTY: &str = "every";
const ARRAY_PROTOTYPE_FILTER_PROPERTY: &str = "filter";
const ARRAY_PROTOTYPE_FIND_PROPERTY: &str = "find";
const ARRAY_PROTOTYPE_FIND_INDEX_PROPERTY: &str = "findIndex";
const ARRAY_PROTOTYPE_FLAT_PROPERTY: &str = "flat";
const ARRAY_PROTOTYPE_FLAT_MAP_PROPERTY: &str = "flatMap";
const ARRAY_PROTOTYPE_FOR_EACH_PROPERTY: &str = "forEach";
const ARRAY_PROTOTYPE_INCLUDES_PROPERTY: &str = "includes";
const ARRAY_PROTOTYPE_INDEX_OF_PROPERTY: &str = "indexOf";
const ARRAY_PROTOTYPE_JOIN_PROPERTY: &str = "join";
const ARRAY_PROTOTYPE_TO_LOCALE_STRING_PROPERTY: &str = "toLocaleString";
const ARRAY_PROTOTYPE_TO_STRING_PROPERTY: &str = "toString";
const ARRAY_PROTOTYPE_LAST_INDEX_OF_PROPERTY: &str = "lastIndexOf";
const ARRAY_PROTOTYPE_MAP_PROPERTY: &str = "map";
const ARRAY_PROTOTYPE_POP_PROPERTY: &str = "pop";
const ARRAY_PROTOTYPE_PUSH_PROPERTY: &str = "push";
const ARRAY_PROTOTYPE_REDUCE_PROPERTY: &str = "reduce";
const ARRAY_PROTOTYPE_REDUCE_RIGHT_PROPERTY: &str = "reduceRight";
const ARRAY_PROTOTYPE_REVERSE_PROPERTY: &str = "reverse";
const ARRAY_PROTOTYPE_SHIFT_PROPERTY: &str = "shift";
const ARRAY_PROTOTYPE_SLICE_PROPERTY: &str = "slice";
const ARRAY_PROTOTYPE_SOME_PROPERTY: &str = "some";
const ARRAY_PROTOTYPE_UNSHIFT_PROPERTY: &str = "unshift";
const ARRAY_PROTOTYPE_SORT_PROPERTY: &str = "sort";
const ARRAY_PROTOTYPE_SPLICE_PROPERTY: &str = "splice";
const ARRAY_PROTOTYPE_FILL_PROPERTY: &str = "fill";
const ARRAY_PROTOTYPE_COPY_WITHIN_PROPERTY: &str = "copyWithin";
const ARRAY_PROTOTYPE_AT_PROPERTY: &str = "at";
const ARRAY_PROTOTYPE_FIND_LAST_PROPERTY: &str = "findLast";
const ARRAY_PROTOTYPE_FIND_LAST_INDEX_PROPERTY: &str = "findLastIndex";
const ARRAY_PROTOTYPE_TO_SORTED_PROPERTY: &str = "toSorted";
const ARRAY_PROTOTYPE_TO_REVERSED_PROPERTY: &str = "toReversed";
const ARRAY_PROTOTYPE_TO_SPLICED_PROPERTY: &str = "toSpliced";
const ARRAY_PROTOTYPE_WITH_PROPERTY: &str = "with";
const ARRAY_PROTOTYPE_VALUES_PROPERTY: &str = "values";
const ARRAY_PROTOTYPE_KEYS_PROPERTY: &str = "keys";
const ARRAY_PROTOTYPE_ENTRIES_PROPERTY: &str = "entries";
const ARRAY_ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const ARRAY_UNSCOPABLES_SYMBOL_DISPLAY: &str = "[Symbol.unscopables]";
const SYMBOL_UNSCOPABLES_PROPERTY: &str = "unscopables";
const ARRAY_UNSCOPABLE_PROPERTIES: &[&str] = &[
    ARRAY_PROTOTYPE_COPY_WITHIN_PROPERTY,
    "entries",
    ARRAY_PROTOTYPE_FILL_PROPERTY,
    ARRAY_PROTOTYPE_FIND_PROPERTY,
    ARRAY_PROTOTYPE_FIND_INDEX_PROPERTY,
    ARRAY_PROTOTYPE_FIND_LAST_PROPERTY,
    ARRAY_PROTOTYPE_FIND_LAST_INDEX_PROPERTY,
    ARRAY_PROTOTYPE_FLAT_PROPERTY,
    ARRAY_PROTOTYPE_FLAT_MAP_PROPERTY,
    ARRAY_PROTOTYPE_INCLUDES_PROPERTY,
    "keys",
    ARRAY_PROTOTYPE_TO_REVERSED_PROPERTY,
    ARRAY_PROTOTYPE_TO_SORTED_PROPERTY,
    ARRAY_PROTOTYPE_TO_SPLICED_PROPERTY,
    ARRAY_PROTOTYPE_VALUES_PROPERTY,
];

/// `Array.prototype` method table installed as non-enumerable data properties.
const ARRAY_PROTOTYPE_METHODS: [(&str, NativeFunctionKind); 35] = [
    (
        ARRAY_PROTOTYPE_CONCAT_PROPERTY,
        NativeFunctionKind::ArrayConcat,
    ),
    (
        ARRAY_PROTOTYPE_EVERY_PROPERTY,
        NativeFunctionKind::ArrayEvery,
    ),
    (
        ARRAY_PROTOTYPE_FILTER_PROPERTY,
        NativeFunctionKind::ArrayFilter,
    ),
    (ARRAY_PROTOTYPE_FIND_PROPERTY, NativeFunctionKind::ArrayFind),
    (
        ARRAY_PROTOTYPE_FIND_INDEX_PROPERTY,
        NativeFunctionKind::ArrayFindIndex,
    ),
    (ARRAY_PROTOTYPE_FLAT_PROPERTY, NativeFunctionKind::ArrayFlat),
    (
        ARRAY_PROTOTYPE_FLAT_MAP_PROPERTY,
        NativeFunctionKind::ArrayFlatMap,
    ),
    (
        ARRAY_PROTOTYPE_FOR_EACH_PROPERTY,
        NativeFunctionKind::ArrayForEach,
    ),
    (
        ARRAY_PROTOTYPE_INCLUDES_PROPERTY,
        NativeFunctionKind::ArrayIncludes,
    ),
    (
        ARRAY_PROTOTYPE_INDEX_OF_PROPERTY,
        NativeFunctionKind::ArrayIndexOf,
    ),
    (ARRAY_PROTOTYPE_JOIN_PROPERTY, NativeFunctionKind::ArrayJoin),
    (
        ARRAY_PROTOTYPE_TO_LOCALE_STRING_PROPERTY,
        NativeFunctionKind::ArrayToLocaleString,
    ),
    (
        ARRAY_PROTOTYPE_TO_STRING_PROPERTY,
        NativeFunctionKind::ArrayToString,
    ),
    (
        ARRAY_PROTOTYPE_LAST_INDEX_OF_PROPERTY,
        NativeFunctionKind::ArrayLastIndexOf,
    ),
    (ARRAY_PROTOTYPE_MAP_PROPERTY, NativeFunctionKind::ArrayMap),
    (ARRAY_PROTOTYPE_POP_PROPERTY, NativeFunctionKind::ArrayPop),
    (ARRAY_PROTOTYPE_PUSH_PROPERTY, NativeFunctionKind::ArrayPush),
    (
        ARRAY_PROTOTYPE_REDUCE_PROPERTY,
        NativeFunctionKind::ArrayReduce,
    ),
    (
        ARRAY_PROTOTYPE_REDUCE_RIGHT_PROPERTY,
        NativeFunctionKind::ArrayReduceRight,
    ),
    (
        ARRAY_PROTOTYPE_REVERSE_PROPERTY,
        NativeFunctionKind::ArrayReverse,
    ),
    (
        ARRAY_PROTOTYPE_SHIFT_PROPERTY,
        NativeFunctionKind::ArrayShift,
    ),
    (
        ARRAY_PROTOTYPE_SLICE_PROPERTY,
        NativeFunctionKind::ArraySlice,
    ),
    (ARRAY_PROTOTYPE_SOME_PROPERTY, NativeFunctionKind::ArraySome),
    (
        ARRAY_PROTOTYPE_UNSHIFT_PROPERTY,
        NativeFunctionKind::ArrayUnshift,
    ),
    (ARRAY_PROTOTYPE_SORT_PROPERTY, NativeFunctionKind::ArraySort),
    (
        ARRAY_PROTOTYPE_SPLICE_PROPERTY,
        NativeFunctionKind::ArraySplice,
    ),
    (ARRAY_PROTOTYPE_FILL_PROPERTY, NativeFunctionKind::ArrayFill),
    (
        ARRAY_PROTOTYPE_COPY_WITHIN_PROPERTY,
        NativeFunctionKind::ArrayCopyWithin,
    ),
    (ARRAY_PROTOTYPE_AT_PROPERTY, NativeFunctionKind::ArrayAt),
    (
        ARRAY_PROTOTYPE_FIND_LAST_PROPERTY,
        NativeFunctionKind::ArrayFindLast,
    ),
    (
        ARRAY_PROTOTYPE_FIND_LAST_INDEX_PROPERTY,
        NativeFunctionKind::ArrayFindLastIndex,
    ),
    (
        ARRAY_PROTOTYPE_TO_SORTED_PROPERTY,
        NativeFunctionKind::ArrayToSorted,
    ),
    (
        ARRAY_PROTOTYPE_TO_REVERSED_PROPERTY,
        NativeFunctionKind::ArrayToReversed,
    ),
    (
        ARRAY_PROTOTYPE_TO_SPLICED_PROPERTY,
        NativeFunctionKind::ArrayToSpliced,
    ),
    (ARRAY_PROTOTYPE_WITH_PROPERTY, NativeFunctionKind::ArrayWith),
];

impl Context {
    pub(super) fn install_array_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (property, kind) in ARRAY_PROTOTYPE_METHODS {
            let method = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, property, method)?;
        }
        let keys = self.create_native_function(NativeFunctionKind::ArrayKeys, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, ARRAY_PROTOTYPE_KEYS_PROPERTY, keys)?;
        let entries =
            self.create_native_function(NativeFunctionKind::ArrayEntries, Value::Undefined)?;
        self.define_non_enumerable_object_property(
            prototype,
            ARRAY_PROTOTYPE_ENTRIES_PROPERTY,
            entries,
        )?;
        let values =
            self.create_native_function(NativeFunctionKind::ArrayValues, Value::Undefined)?;
        self.define_non_enumerable_object_property(
            prototype,
            ARRAY_PROTOTYPE_VALUES_PROPERTY,
            values.clone(),
        )?;
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ARRAY_ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(values),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_array_unscopables(prototype)?;
        Ok(())
    }

    fn install_array_unscopables(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let Value::Symbol(symbol) =
            self.get_named(&symbol_constructor, SYMBOL_UNSCOPABLES_PROPERTY)?
        else {
            return Err(Error::runtime("Symbol.unscopables is not a symbol"));
        };
        let Value::Object(list) = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?
        else {
            return Err(Error::runtime("array unscopables list is not an object"));
        };
        for property in ARRAY_UNSCOPABLE_PROPERTIES {
            let key = self.intern_property_key(property)?;
            self.objects.define_property(
                list,
                key,
                property,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(Value::Bool(true)),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            ARRAY_UNSCOPABLES_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(list)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
