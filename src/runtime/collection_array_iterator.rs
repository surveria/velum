use crate::{
    error::Result,
    runtime::{Context, collections::CollectionIteratorState},
    value::Value,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum ArrayIterationTarget {
    Keys,
    Values,
    Entries,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct LiveArrayIteratorState {
    pub(in crate::runtime) owner: Value,
    pub(in crate::runtime) target: ArrayIterationTarget,
    pub(in crate::runtime) cursor: usize,
    pub(in crate::runtime) done: bool,
}

impl Context {
    pub(in crate::runtime) fn create_live_array_iterator(
        &mut self,
        owner: Value,
        target: ArrayIterationTarget,
    ) -> Result<crate::runtime::collections::CollectionIteratorId> {
        self.insert_iterator_state(CollectionIteratorState::LiveArray(LiveArrayIteratorState {
            owner,
            target,
            cursor: 0,
            done: false,
        }))
    }
}
