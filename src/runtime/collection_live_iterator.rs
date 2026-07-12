use crate::{
    error::{Error, Result},
    runtime::collections::{
        CollectionIterationTarget, CollectionIteratorId, CollectionIteratorState, CollectionKind,
    },
    value::Value,
};

use super::Context;

const COLLECTION_ITERATOR_RECEIVER_ERROR: &str =
    "Collection Iterator.prototype.next requires a compatible iterator receiver";

#[derive(Clone, Copy, Eq, PartialEq)]
enum CollectionIteratorBrand {
    Snapshot,
    Map,
    Set,
    RegExpString,
}

impl Context {
    pub(in crate::runtime) fn collection_iterator_step_for_receiver(
        &mut self,
        requested: CollectionIteratorId,
        actual: CollectionIteratorId,
    ) -> Result<Option<Value>> {
        let requested_brand = self.collection_iterator_brand(requested)?;
        if requested_brand != self.collection_iterator_brand(actual)? {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        }
        if requested_brand == CollectionIteratorBrand::Snapshot {
            return self.collection_iterator_step(actual);
        }
        if requested_brand == CollectionIteratorBrand::RegExpString {
            return self.regexp_string_iterator_step(actual);
        }
        self.live_collection_iterator_step(actual)
    }

    fn collection_iterator_brand(
        &self,
        iterator: CollectionIteratorId,
    ) -> Result<CollectionIteratorBrand> {
        match self.collection_iterators.get(iterator.index()) {
            Some(CollectionIteratorState::Snapshot(_)) => Ok(CollectionIteratorBrand::Snapshot),
            Some(CollectionIteratorState::LiveCollection(state)) => Ok(match state.kind {
                CollectionKind::Map => CollectionIteratorBrand::Map,
                CollectionKind::Set => CollectionIteratorBrand::Set,
                CollectionKind::WeakMap
                | CollectionKind::WeakSet
                | CollectionKind::AsyncDisposableStack
                | CollectionKind::DisposableStack => {
                    return Err(Error::runtime(
                        "weak collection cannot have a live iterator",
                    ));
                }
            }),
            Some(CollectionIteratorState::RegExpString(_)) => {
                Ok(CollectionIteratorBrand::RegExpString)
            }
            Some(_) => Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR)),
            None => Err(Error::runtime("collection iterator disappeared")),
        }
    }

    fn live_collection_iterator_step(
        &mut self,
        iterator: CollectionIteratorId,
    ) -> Result<Option<Value>> {
        let Some(CollectionIteratorState::LiveCollection(state)) =
            self.collection_iterators.get(iterator.index())
        else {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        };
        if state.done {
            return Ok(None);
        }
        let collection = state.collection;
        let target = state.target;
        let cursor = state.cursor;
        let Some((index, key, value)) = self.collection_entry_at_or_after(collection, cursor)?
        else {
            self.finish_live_collection_iterator(iterator)?;
            return Ok(None);
        };
        self.advance_live_collection_iterator(iterator, index)?;
        match target {
            CollectionIterationTarget::Keys => Ok(Some(key)),
            CollectionIterationTarget::Values => Ok(Some(value)),
            CollectionIterationTarget::Entries => {
                self.create_array_from_elements(vec![key, value]).map(Some)
            }
        }
    }

    fn finish_live_collection_iterator(&mut self, iterator: CollectionIteratorId) -> Result<()> {
        let Some(CollectionIteratorState::LiveCollection(state)) =
            self.collection_iterators.get_mut(iterator.index())
        else {
            return Err(Error::runtime("live collection iterator disappeared"));
        };
        state.done = true;
        Ok(())
    }

    fn advance_live_collection_iterator(
        &mut self,
        iterator: CollectionIteratorId,
        index: usize,
    ) -> Result<()> {
        let Some(CollectionIteratorState::LiveCollection(state)) =
            self.collection_iterators.get_mut(iterator.index())
        else {
            return Err(Error::runtime("live collection iterator disappeared"));
        };
        state.cursor = index
            .checked_add(1)
            .ok_or_else(|| Error::limit("collection iterator cursor overflowed"))?;
        Ok(())
    }
}
