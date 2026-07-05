use crate::{
    error::{Error, Result},
    value::Value,
};

use super::ARRAY_INDEX_LIMIT_ERROR;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ArrayLength(pub(super) u32);

impl ArrayLength {
    pub(super) fn from_usize(value: usize) -> Result<Self> {
        let value = u32::try_from(value)
            .map_err(|_| Error::limit("array length exceeded supported range"))?;
        Ok(Self(value))
    }

    pub(super) fn value(self) -> Value {
        Value::Number(f64::from(self.0))
    }

    pub(super) const fn contains(self, index: ArrayIndex) -> bool {
        index.0 < self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ArrayIndex(pub(super) u32);

impl ArrayIndex {
    pub(super) fn from_u32(value: u32) -> Result<Self> {
        if value == u32::MAX {
            return Err(Error::limit(ARRAY_INDEX_LIMIT_ERROR));
        }
        Ok(Self(value))
    }

    pub(super) fn from_usize(value: usize) -> Result<Self> {
        let value = u32::try_from(value).map_err(|_| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        Self::from_u32(value)
    }

    pub(super) fn parse(property: &str) -> Option<Self> {
        let value = property.parse::<u32>().ok()?;
        if value == u32::MAX || value.to_string() != property {
            return None;
        }
        Some(Self(value))
    }

    pub(super) fn key(self) -> String {
        self.0.to_string()
    }

    pub(super) fn position(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit(ARRAY_INDEX_LIMIT_ERROR))
    }

    pub(super) fn dense_position(self, max_properties: usize) -> Result<Option<usize>> {
        let position = self.position()?;
        if position < max_properties {
            return Ok(Some(position));
        }
        Ok(None)
    }

    pub(super) fn next_length(self) -> Result<ArrayLength> {
        self.0
            .checked_add(1)
            .map(ArrayLength)
            .ok_or_else(|| Error::limit("array length exceeded supported range"))
    }

    pub(super) const fn length(self) -> ArrayLength {
        ArrayLength(self.0)
    }
}
