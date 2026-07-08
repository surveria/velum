use crate::{
    error::{Error, Result},
    runtime::Context,
    value::Value,
};

const ARRAY_ITERATOR_LIMIT_ERROR: &str = "array iterator count exceeded";
const ARRAY_ITERATOR_MISSING_ERROR: &str = "array iterator disappeared";
const ARRAY_ITERATOR_INDEX_LIMIT_ERROR: &str = "array iterator index overflowed";

/// VM-local index of one live Array iterator.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) struct ArrayIteratorId(usize);

impl ArrayIteratorId {
    const fn index(self) -> usize {
        self.0
    }
}

/// Which value shape an Array iterator yields.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum ArrayIterationTarget {
    Keys,
    Values,
    Entries,
}

/// Live Array iterator cursor. Length and indexed values are read by the
/// native `next` implementation at each step so receiver mutations are visible.
#[derive(Debug, Clone)]
pub(in crate::runtime) struct ArrayIteratorState {
    source: Value,
    index: usize,
    target: ArrayIterationTarget,
}

impl ArrayIteratorState {
    const fn new(source: Value, target: ArrayIterationTarget) -> Self {
        Self {
            source,
            index: 0,
            target,
        }
    }
}

impl Context {
    pub(in crate::runtime) fn create_array_iterator(
        &mut self,
        source: Value,
        target: ArrayIterationTarget,
    ) -> Result<ArrayIteratorId> {
        if self.array_iterators.len() >= self.limits.max_objects {
            return Err(Error::limit(format!(
                "{ARRAY_ITERATOR_LIMIT_ERROR} {}",
                self.limits.max_objects
            )));
        }
        let id = ArrayIteratorId(self.array_iterators.len());
        self.array_iterators
            .push(ArrayIteratorState::new(source, target));
        Ok(id)
    }

    pub(in crate::runtime) fn array_iterator_snapshot(
        &self,
        id: ArrayIteratorId,
    ) -> Result<(Value, usize, ArrayIterationTarget)> {
        let state = self
            .array_iterators
            .get(id.index())
            .ok_or_else(|| Error::runtime(ARRAY_ITERATOR_MISSING_ERROR))?;
        Ok((state.source.clone(), state.index, state.target))
    }

    pub(in crate::runtime) fn advance_array_iterator(&mut self, id: ArrayIteratorId) -> Result<()> {
        let state = self
            .array_iterators
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime(ARRAY_ITERATOR_MISSING_ERROR))?;
        state.index = state
            .index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_ITERATOR_INDEX_LIMIT_ERROR))?;
        Ok(())
    }
}
