use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        collections::{CollectionIteratorId, CollectionIteratorState},
    },
    value::Value,
};

#[derive(Debug, Clone)]
pub(in crate::runtime) struct RegExpStringIteratorState {
    pub(in crate::runtime) matcher: Value,
    pub(in crate::runtime) input: Value,
    pub(in crate::runtime) global: bool,
    pub(in crate::runtime) unicode: bool,
    pub(in crate::runtime) done: bool,
}

impl Context {
    pub(in crate::runtime) fn create_regexp_string_iterator(
        &mut self,
        matcher: Value,
        input: Value,
        global: bool,
        unicode: bool,
    ) -> Result<CollectionIteratorId> {
        self.insert_iterator_state(CollectionIteratorState::RegExpString(
            RegExpStringIteratorState {
                matcher,
                input,
                global,
                unicode,
                done: false,
            },
        ))
    }

    pub(in crate::runtime) fn regexp_string_iterator_state(
        &self,
        id: CollectionIteratorId,
    ) -> Result<&RegExpStringIteratorState> {
        let CollectionIteratorState::RegExpString(state) = self
            .collection_iterators
            .get(id.index())
            .ok_or_else(|| Error::runtime("RegExp string iterator state disappeared"))?
        else {
            return Err(Error::runtime(
                "iterator state is not a RegExp string iterator",
            ));
        };
        Ok(state)
    }

    pub(in crate::runtime) fn finish_regexp_string_iterator(
        &mut self,
        id: CollectionIteratorId,
    ) -> Result<()> {
        let CollectionIteratorState::RegExpString(state) = self
            .collection_iterators
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("RegExp string iterator state disappeared"))?
        else {
            return Err(Error::runtime(
                "iterator state is not a RegExp string iterator",
            ));
        };
        state.done = true;
        Ok(())
    }
}
