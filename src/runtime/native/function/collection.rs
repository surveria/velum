use crate::{
    error::Result,
    runtime::{
        Context, call::RuntimeCallArgs, collections::CollectionKind,
        native::CollectionIterationTarget,
    },
    value::Value,
};

use super::NativeFunctionKind;

impl Context {
    pub(in crate::runtime) fn eval_collection_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        let result = match kind {
            NativeFunctionKind::Map
            | NativeFunctionKind::Set
            | NativeFunctionKind::WeakMap
            | NativeFunctionKind::WeakSet => Self::eval_collection_constructor_call(),
            NativeFunctionKind::MapGet => self.eval_map_get(args, this_value),
            NativeFunctionKind::MapSet => self.eval_map_set(args, this_value),
            NativeFunctionKind::MapHas => {
                self.eval_collection_has(CollectionKind::Map, args, this_value)
            }
            NativeFunctionKind::MapDelete => {
                self.eval_collection_delete(CollectionKind::Map, args, this_value)
            }
            NativeFunctionKind::MapClear => {
                self.eval_collection_clear(CollectionKind::Map, this_value)
            }
            NativeFunctionKind::MapForEach => {
                self.eval_collection_for_each(CollectionKind::Map, args, this_value)
            }
            NativeFunctionKind::MapSizeGetter => {
                self.eval_collection_size(CollectionKind::Map, this_value)
            }
            NativeFunctionKind::MapEntries => self.eval_collection_iterator(
                CollectionKind::Map,
                CollectionIterationTarget::Entries,
                this_value,
            ),
            NativeFunctionKind::MapKeys => self.eval_collection_iterator(
                CollectionKind::Map,
                CollectionIterationTarget::Keys,
                this_value,
            ),
            NativeFunctionKind::MapValues => self.eval_collection_iterator(
                CollectionKind::Map,
                CollectionIterationTarget::Values,
                this_value,
            ),
            NativeFunctionKind::SetAdd => self.eval_set_add(args, this_value),
            NativeFunctionKind::SetHas => {
                self.eval_collection_has(CollectionKind::Set, args, this_value)
            }
            NativeFunctionKind::SetDelete => {
                self.eval_collection_delete(CollectionKind::Set, args, this_value)
            }
            NativeFunctionKind::SetClear => {
                self.eval_collection_clear(CollectionKind::Set, this_value)
            }
            NativeFunctionKind::SetForEach => {
                self.eval_collection_for_each(CollectionKind::Set, args, this_value)
            }
            NativeFunctionKind::SetSizeGetter => {
                self.eval_collection_size(CollectionKind::Set, this_value)
            }
            NativeFunctionKind::SetEntries => self.eval_collection_iterator(
                CollectionKind::Set,
                CollectionIterationTarget::Entries,
                this_value,
            ),
            NativeFunctionKind::SetValues => self.eval_collection_iterator(
                CollectionKind::Set,
                CollectionIterationTarget::Values,
                this_value,
            ),
            NativeFunctionKind::CollectionIteratorNext(iterator) => {
                self.eval_collection_iterator_next(iterator)
            }
            NativeFunctionKind::IteratorSelf => Ok(this_value.clone()),
            NativeFunctionKind::WeakMapGet => self.eval_weak_map_get(args, this_value),
            NativeFunctionKind::WeakMapSet => self.eval_weak_map_set(args, this_value),
            NativeFunctionKind::WeakMapHas => {
                self.eval_weak_collection_has(CollectionKind::WeakMap, args, this_value)
            }
            NativeFunctionKind::WeakMapDelete => {
                self.eval_weak_collection_delete(CollectionKind::WeakMap, args, this_value)
            }
            NativeFunctionKind::WeakSetAdd => self.eval_weak_set_add(args, this_value),
            NativeFunctionKind::WeakSetHas => {
                self.eval_weak_collection_has(CollectionKind::WeakSet, args, this_value)
            }
            NativeFunctionKind::WeakSetDelete => {
                self.eval_weak_collection_delete(CollectionKind::WeakSet, args, this_value)
            }
            _ => return None,
        };
        Some(result)
    }
}
