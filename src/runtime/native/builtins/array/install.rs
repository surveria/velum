use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::NativeFunctionKind;

const ARRAY_FROM_PROPERTY: &str = "from";
const ARRAY_IS_ARRAY_PROPERTY: &str = "isArray";
const ARRAY_OF_PROPERTY: &str = "of";
const ARRAY_PROTOTYPE_CONCAT_PROPERTY: &str = "concat";
const ARRAY_PROTOTYPE_ENTRIES_PROPERTY: &str = "entries";
const ARRAY_PROTOTYPE_EVERY_PROPERTY: &str = "every";
const ARRAY_PROTOTYPE_FILTER_PROPERTY: &str = "filter";
const ARRAY_PROTOTYPE_FIND_PROPERTY: &str = "find";
const ARRAY_PROTOTYPE_FIND_INDEX_PROPERTY: &str = "findIndex";
const ARRAY_PROTOTYPE_FOR_EACH_PROPERTY: &str = "forEach";
const ARRAY_PROTOTYPE_INCLUDES_PROPERTY: &str = "includes";
const ARRAY_PROTOTYPE_INDEX_OF_PROPERTY: &str = "indexOf";
const ARRAY_PROTOTYPE_JOIN_PROPERTY: &str = "join";
const ARRAY_PROTOTYPE_KEYS_PROPERTY: &str = "keys";
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
const ARRAY_PROTOTYPE_VALUES_PROPERTY: &str = "values";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";

impl Context {
    pub(in crate::runtime::native::builtins::array) fn array_method_value(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<Value> {
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    pub(in crate::runtime::native) fn install_array_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        let methods = [
            (
                ARRAY_PROTOTYPE_CONCAT_PROPERTY,
                NativeFunctionKind::ArrayConcat,
            ),
            (
                ARRAY_PROTOTYPE_ENTRIES_PROPERTY,
                NativeFunctionKind::ArrayEntries,
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
            (ARRAY_PROTOTYPE_KEYS_PROPERTY, NativeFunctionKind::ArrayKeys),
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
            (
                ARRAY_PROTOTYPE_VALUES_PROPERTY,
                NativeFunctionKind::ArrayValues,
            ),
        ];
        for (property, kind) in methods {
            let method = self.array_method_value(kind)?;
            self.define_non_enumerable_object_property(prototype, property, method)?;
        }
        self.install_array_prototype_symbol_iterator(prototype)
    }

    pub(in crate::runtime::native) fn install_array_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let methods = [
            (ARRAY_FROM_PROPERTY, NativeFunctionKind::ArrayFrom),
            (ARRAY_IS_ARRAY_PROPERTY, NativeFunctionKind::ArrayIsArray),
            (ARRAY_OF_PROPERTY, NativeFunctionKind::ArrayOf),
        ];
        for (property, kind) in methods {
            let method = self.array_method_value(kind)?;
            let key = self.intern_property_key(property)?;
            self.native_function_mut(constructor)?
                .properties_mut()
                .define_builtin(key, method, PropertyEnumerable::No);
        }
        Ok(())
    }

    fn install_array_prototype_symbol_iterator(&mut self, prototype: ObjectId) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let method = self.array_method_value(NativeFunctionKind::ArrayValues)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
