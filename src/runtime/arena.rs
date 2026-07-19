#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{Error, Result};

/// Non-moving indexed storage with explicit vacant slots.
///
/// VM-local ids remain stable while a record is live. A stop-the-world
/// collector may remove unreachable records and later reuse their indices
/// after identity-bearing optimization caches have been invalidated.
#[derive(Debug, Clone)]
pub struct SlotArena<T> {
    slots: Vec<Option<T>>,
    free: Vec<usize>,
    live: usize,
}

impl<T> SlotArena<T> {
    pub(crate) const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> Result<usize> {
        self.reserve_insert()?;
        if let Some(index) = self.free.pop() {
            let Some(slot) = self.slots.get_mut(index) else {
                return Err(Error::runtime("arena free slot is not defined"));
            };
            if slot.is_some() {
                return Err(Error::runtime("arena free slot is occupied"));
            }
            *slot = Some(value);
            self.live = self
                .live
                .checked_add(1)
                .ok_or_else(|| Error::limit("arena live count overflowed"))?;
            return Ok(index);
        }

        let index = self.slots.len();
        self.slots.push(Some(value));
        self.live = self
            .live
            .checked_add(1)
            .ok_or_else(|| Error::limit("arena live count overflowed"))?;
        Ok(index)
    }

    pub(crate) fn reserve_insert(&mut self) -> Result<()> {
        self.live
            .checked_add(1)
            .ok_or_else(|| Error::limit("arena live count overflowed"))?;
        if self.free.is_empty() {
            self.slots
                .try_reserve(1)
                .map_err(|error| Error::limit(format!("arena storage exhausted: {error}")))?;
        }
        Ok(())
    }

    pub(crate) fn next_index(&self) -> usize {
        self.free.last().copied().unwrap_or(self.slots.len())
    }

    pub(crate) fn insert_at_next(&mut self, expected: usize, value: T) -> Result<()> {
        let actual = self.insert(value)?;
        if actual == expected {
            return Ok(());
        }
        Err(Error::runtime(format!(
            "arena insertion index changed from {expected} to {actual}"
        )))
    }

    pub(crate) fn reserve_removals(&mut self, additional: usize) -> Result<()> {
        self.free
            .try_reserve(additional)
            .map_err(|error| Error::limit(format!("arena free-list storage exhausted: {error}")))
    }

    pub(crate) fn remove_reserved(&mut self, index: usize) -> Result<Option<T>> {
        let Some(slot) = self.slots.get_mut(index) else {
            return Ok(None);
        };
        let Some(value) = slot.take() else {
            return Ok(None);
        };
        self.live = self
            .live
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("arena live count underflowed"))?;
        self.free.push(index);
        Ok(Some(value))
    }

    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.slots.get(index).and_then(Option::as_ref)
    }

    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.slots.get_mut(index).and_then(Option::as_mut)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.slots.iter().filter_map(Option::as_ref)
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.slots.iter_mut().filter_map(Option::as_mut)
    }

    pub(crate) fn indexed(&self) -> impl Iterator<Item = (usize, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_ref().map(|value| (index, value)))
    }

    pub(crate) fn indexed_mut(&mut self) -> impl Iterator<Item = (usize, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, slot)| slot.as_mut().map(|value| (index, value)))
    }

    pub(crate) fn sweep_unmarked(&mut self, marks: &[bool]) -> Result<usize> {
        if marks.len() != self.slots.len() {
            return Err(Error::runtime("arena mark bitmap length mismatch"));
        }
        let removed = self
            .indexed()
            .filter(|(index, _value)| !marks.get(*index).copied().unwrap_or(false))
            .count();
        self.reserve_removals(removed)?;
        for index in 0..self.slots.len() {
            if marks.get(index).copied().unwrap_or(false) || self.get(index).is_none() {
                continue;
            }
            let removed_value = self.remove_reserved(index)?;
            if removed_value.is_none() {
                return Err(Error::runtime(
                    "marked arena record disappeared during sweep",
                ));
            }
        }
        Ok(removed)
    }

    pub(crate) const fn len(&self) -> usize {
        self.live
    }

    pub(crate) const fn slot_len(&self) -> usize {
        self.slots.len()
    }
}

impl<T> Default for SlotArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'arena, T> IntoIterator for &'arena SlotArena<T> {
    type Item = &'arena T;
    type IntoIter = core::iter::FilterMap<
        core::slice::Iter<'arena, Option<T>>,
        fn(&'arena Option<T>) -> Option<&'arena T>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.slots.iter().filter_map(Option::as_ref)
    }
}

impl<'arena, T> IntoIterator for &'arena mut SlotArena<T> {
    type Item = &'arena mut T;
    type IntoIter = core::iter::FilterMap<
        core::slice::IterMut<'arena, Option<T>>,
        fn(&'arena mut Option<T>) -> Option<&'arena mut T>,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.slots.iter_mut().filter_map(Option::as_mut)
    }
}
