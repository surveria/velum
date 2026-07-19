#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::collections::BTreeMap;

use crate::value::Value;
use crate::{
    error::{Error, Result},
    runtime::trace::{StrongEdgeReference, StrongEdgeVisitor, VmObjectEdgeKind},
};

use super::super::{PropertyEnumerable, PropertyKey};
use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayIndex, ObjectProperty};

#[derive(Debug, Clone)]
pub(in crate::runtime::object) struct ArrayStorage {
    elements: ArrayElements,
    sparse_keys: BTreeMap<ArrayIndex, PropertyKey>,
    property_count: usize,
}

pub(in crate::runtime::object) enum ShiftedArrayElement {
    Hole,
    Property(ObjectProperty),
}

impl ShiftedArrayElement {
    pub(in crate::runtime::object) fn into_value(self) -> Value {
        match self {
            Self::Hole => Value::Undefined,
            Self::Property(property) => property.value(),
        }
    }
}

impl ArrayStorage {
    pub(in crate::runtime::object) const fn new() -> Self {
        Self {
            elements: ArrayElements::Packed(Vec::new()),
            sparse_keys: BTreeMap::new(),
            property_count: 0,
        }
    }

    pub(in crate::runtime::object) const fn property_count(&self) -> usize {
        self.property_count
    }

    pub(in crate::runtime::object) fn dense_property(
        &self,
        index: ArrayIndex,
    ) -> Option<&ObjectProperty> {
        let position = index.position().ok()?;
        self.dense_property_at_position(position)
    }

    pub(in crate::runtime::object) fn dense_property_mut(
        &mut self,
        index: ArrayIndex,
    ) -> Result<Option<&mut ObjectProperty>> {
        let position = index.position()?;
        Ok(match &mut self.elements {
            ArrayElements::Packed(elements) => elements.get_mut(position),
            ArrayElements::Holey(elements) => elements.get_mut(position).and_then(Option::as_mut),
        })
    }

    pub(in crate::runtime::object) fn dense_property_at_position(
        &self,
        position: usize,
    ) -> Option<&ObjectProperty> {
        match &self.elements {
            ArrayElements::Packed(elements) => elements.get(position),
            ArrayElements::Holey(elements) => elements.get(position).and_then(Option::as_ref),
        }
    }

    pub(in crate::runtime::object) fn packed_properties_for_len(
        &self,
        len: usize,
    ) -> Option<&[ObjectProperty]> {
        if self.has_sparse_keys() {
            return None;
        }
        match &self.elements {
            ArrayElements::Packed(elements) if elements.len() == len => Some(elements.as_slice()),
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => None,
        }
    }

    pub(in crate::runtime::object) fn holey_properties_for_len(
        &self,
        len: usize,
    ) -> Option<&[Option<ObjectProperty>]> {
        if self.has_sparse_keys() {
            return None;
        }
        match &self.elements {
            ArrayElements::Holey(elements) if elements.len() == len => Some(elements),
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => None,
        }
    }

    pub(in crate::runtime::object) fn has_dense_property_in_range(
        &self,
        start: usize,
        end: usize,
    ) -> bool {
        if end <= start {
            return false;
        }
        match &self.elements {
            ArrayElements::Packed(elements) => start < end.min(elements.len()),
            ArrayElements::Holey(elements) => {
                let count = end.saturating_sub(start);
                elements.iter().skip(start).take(count).any(Option::is_some)
            }
        }
    }

    pub(in crate::runtime::object) fn seal_dense_properties(&mut self) {
        match &mut self.elements {
            ArrayElements::Packed(elements) => {
                for property in elements {
                    property.seal();
                }
            }
            ArrayElements::Holey(elements) => {
                for property in elements.iter_mut().flatten() {
                    property.seal();
                }
            }
        }
    }

    pub(in crate::runtime::object) fn freeze_dense_properties(&mut self) {
        match &mut self.elements {
            ArrayElements::Packed(elements) => {
                for property in elements {
                    property.freeze();
                }
            }
            ArrayElements::Holey(elements) => {
                for property in elements.iter_mut().flatten() {
                    property.freeze();
                }
            }
        }
    }

    pub(in crate::runtime::object) fn dense_properties_are_sealed(&self) -> bool {
        match &self.elements {
            ArrayElements::Packed(elements) => {
                elements.iter().all(|property| !property.is_configurable())
            }
            ArrayElements::Holey(elements) => elements
                .iter()
                .flatten()
                .all(|property| !property.is_configurable()),
        }
    }

    pub(in crate::runtime::object) fn dense_properties_are_frozen(&self) -> bool {
        match &self.elements {
            ArrayElements::Packed(elements) => elements.iter().all(ObjectProperty::is_frozen),
            ArrayElements::Holey(elements) => {
                elements.iter().flatten().all(ObjectProperty::is_frozen)
            }
        }
    }

    pub(in crate::runtime::object) fn has_sparse_key_in_range(
        &self,
        start: usize,
        end: usize,
    ) -> Result<bool> {
        if end <= start {
            return Ok(false);
        }
        for index in self.sparse_keys.keys() {
            let position = index.position()?;
            if position >= start && position < end {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::runtime::object) fn reverse_dense_for_len_if_default(
        &mut self,
        len: usize,
    ) -> bool {
        if self.has_sparse_keys() {
            return false;
        }
        match &mut self.elements {
            ArrayElements::Packed(elements) if elements.len() == len => {
                if !elements
                    .iter()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return false;
                }
                elements.reverse();
                true
            }
            ArrayElements::Holey(elements) if elements.len() == len => {
                if !elements
                    .iter()
                    .flatten()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return false;
                }
                elements.reverse();
                true
            }
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => false,
        }
    }

    pub(in crate::runtime::object) fn sort_packed_default_numbers_for_len(
        &mut self,
        len: usize,
        descending: bool,
    ) -> bool {
        if self.has_sparse_keys() {
            return false;
        }
        let ArrayElements::Packed(elements) = &mut self.elements else {
            return false;
        };
        if elements.len() != len
            || !elements.iter().all(|property| {
                Self::default_number_property_value(property).is_some_and(|number| !number.is_nan())
            })
        {
            return false;
        }
        elements.sort_by(|left, right| {
            let Some(left) = Self::default_number_property_value(left) else {
                return core::cmp::Ordering::Equal;
            };
            let Some(right) = Self::default_number_property_value(right) else {
                return core::cmp::Ordering::Equal;
            };
            Self::numeric_sort_ordering(left, right, descending)
        });
        true
    }

    fn default_number_property_value(property: &ObjectProperty) -> Option<f64> {
        if !property.has_default_array_attributes() {
            return None;
        }
        let Value::Number(number) = property.data_value_ref()? else {
            return None;
        };
        Some(*number)
    }

    fn numeric_sort_ordering(left: f64, right: f64, descending: bool) -> core::cmp::Ordering {
        let result = if descending {
            right - left
        } else {
            left - right
        };
        if result.is_nan() || result == 0.0 {
            return core::cmp::Ordering::Equal;
        }
        if result < 0.0 {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Greater
        }
    }

    pub(in crate::runtime::object) fn shift_dense_for_len_if_default(
        &mut self,
        len: usize,
        allow_holey: bool,
    ) -> Option<ShiftedArrayElement> {
        if self.has_sparse_keys() {
            return None;
        }
        match &mut self.elements {
            ArrayElements::Packed(elements) if elements.len() == len => {
                if !elements
                    .iter()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return None;
                }
                if elements.is_empty() {
                    return None;
                }
                let removed = elements.remove(0);
                self.property_count = self.property_count.saturating_sub(1);
                Some(ShiftedArrayElement::Property(removed))
            }
            ArrayElements::Holey(elements) if allow_holey && elements.len() == len => {
                if !elements
                    .iter()
                    .flatten()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return None;
                }
                if elements.is_empty() {
                    return Some(ShiftedArrayElement::Hole);
                }
                let removed = elements.drain(0..1).next().flatten();
                if removed.is_some() {
                    self.property_count = self.property_count.saturating_sub(1);
                }
                Some(removed.map_or(ShiftedArrayElement::Hole, ShiftedArrayElement::Property))
            }
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => None,
        }
    }

    pub(in crate::runtime::object) fn unshift_dense_for_len_if_default(
        &mut self,
        len: usize,
        values: &[Value],
        max_properties: usize,
        allow_holey: bool,
    ) -> bool {
        if self.has_sparse_keys() {
            return false;
        }
        let Some(new_property_count) = self.property_count.checked_add(values.len()) else {
            return false;
        };
        let Some(new_len) = len.checked_add(values.len()) else {
            return false;
        };
        if new_property_count > max_properties || new_len > max_properties {
            return false;
        }
        match &mut self.elements {
            ArrayElements::Packed(elements) if elements.len() == len => {
                if !elements
                    .iter()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return false;
                }
                let properties = values
                    .iter()
                    .cloned()
                    .map(|value| ObjectProperty::ordinary(value, PropertyEnumerable::Yes));
                elements.splice(0..0, properties);
            }
            ArrayElements::Holey(elements) if allow_holey && elements.len() == len => {
                if !elements
                    .iter()
                    .flatten()
                    .all(ObjectProperty::has_default_array_attributes)
                {
                    return false;
                }
                let properties = values
                    .iter()
                    .cloned()
                    .map(|value| Some(ObjectProperty::ordinary(value, PropertyEnumerable::Yes)));
                elements.splice(0..0, properties);
            }
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => return false,
        }
        self.property_count = new_property_count;
        true
    }

    pub(in crate::runtime::object) fn is_holey_dense_for_len(&self, len: usize) -> bool {
        if self.has_sparse_keys() {
            return false;
        }
        matches!(&self.elements, ArrayElements::Holey(elements) if elements.len() == len)
    }

    pub(in crate::runtime::object) fn pop_packed_for_len_if_configurable(
        &mut self,
        len: usize,
    ) -> Option<ObjectProperty> {
        if self.has_sparse_keys() {
            return None;
        }
        let ArrayElements::Packed(elements) = &mut self.elements else {
            return None;
        };
        if elements.len() != len {
            return None;
        }
        let property = elements.last()?;
        if !property.is_configurable() {
            return None;
        }
        let removed = elements.pop()?;
        self.property_count = self.property_count.saturating_sub(1);
        Some(removed)
    }

    pub(in crate::runtime::object) fn append_packed_default_value_iter(
        &mut self,
        values: impl IntoIterator<Item = Value>,
        value_count: usize,
        max_properties: usize,
    ) -> Result<usize> {
        if self.has_sparse_keys() {
            return Err(Error::runtime("packed array storage has sparse keys"));
        }
        let Some(new_property_count) = self.property_count.checked_add(value_count) else {
            return Err(Error::limit("object property count overflowed"));
        };
        if new_property_count > max_properties {
            return Err(Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        let ArrayElements::Packed(elements) = &mut self.elements else {
            return Err(Error::runtime("array storage is not packed"));
        };
        for value in values {
            elements.push(ObjectProperty::ordinary(value, PropertyEnumerable::Yes));
        }
        self.property_count = new_property_count;
        Ok(value_count)
    }

    pub(in crate::runtime::object) const fn dense_len(&self) -> usize {
        match &self.elements {
            ArrayElements::Packed(elements) => elements.len(),
            ArrayElements::Holey(elements) => elements.len(),
        }
    }

    pub(in crate::runtime::object) fn indices_at_or_above(
        &self,
        start: usize,
    ) -> Result<Vec<ArrayIndex>> {
        let mut indices = Vec::new();
        match &self.elements {
            ArrayElements::Packed(elements) => {
                for (position, _) in elements.iter().enumerate().skip(start) {
                    indices.push(ArrayIndex::from_usize(position)?);
                }
            }
            ArrayElements::Holey(elements) => {
                for (position, element) in elements.iter().enumerate().skip(start) {
                    if element.is_some() {
                        indices.push(ArrayIndex::from_usize(position)?);
                    }
                }
            }
        }
        for index in self.sparse_keys.keys() {
            if index.position()? >= start {
                indices.push(*index);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        Ok(indices)
    }

    pub(in crate::runtime::object) fn insert_dense_property(
        &mut self,
        index: ArrayIndex,
        property: ObjectProperty,
    ) -> Result<Option<ObjectProperty>> {
        let position = index.position()?;
        match &mut self.elements {
            ArrayElements::Packed(elements) => {
                if let Some(existing) = elements.get_mut(position) {
                    return Ok(Some(core::mem::replace(existing, property)));
                }
                if position == elements.len() {
                    elements.push(property);
                    self.property_count = self.property_count.saturating_add(1);
                    return Ok(None);
                }
                let mut holey = Vec::with_capacity(Self::checked_dense_len(position)?);
                holey.extend(elements.drain(..).map(Some));
                holey.resize_with(Self::checked_dense_len(position)?, || None);
                let slot = holey
                    .get_mut(position)
                    .ok_or_else(|| Error::runtime("array index storage is not available"))?;
                *slot = Some(property);
                self.elements = ArrayElements::Holey(holey);
                self.property_count = self.property_count.saturating_add(1);
                Ok(None)
            }
            ArrayElements::Holey(elements) => {
                if elements.get(position).is_none() {
                    elements.resize_with(Self::checked_dense_len(position)?, || None);
                }
                let slot = elements
                    .get_mut(position)
                    .ok_or_else(|| Error::runtime("array index storage is not available"))?;
                let previous = slot.replace(property);
                if previous.is_none() {
                    self.property_count = self.property_count.saturating_add(1);
                }
                Ok(previous)
            }
        }
    }

    pub(in crate::runtime::object) fn remove_dense_property(
        &mut self,
        index: ArrayIndex,
    ) -> Result<Option<ObjectProperty>> {
        let position = index.position()?;
        let removed = match &mut self.elements {
            ArrayElements::Packed(elements) => {
                if elements.get(position).is_none() {
                    return Ok(None);
                }
                if position.checked_add(1) == Some(elements.len()) {
                    elements.pop()
                } else {
                    let mut holey = Vec::with_capacity(elements.len());
                    holey.extend(elements.drain(..).map(Some));
                    let removed = holey.get_mut(position).and_then(Option::take);
                    self.elements = ArrayElements::Holey(holey);
                    removed
                }
            }
            ArrayElements::Holey(elements) => {
                let Some(slot) = elements.get_mut(position) else {
                    return Ok(None);
                };
                slot.take()
            }
        };
        if removed.is_some() {
            self.property_count = self.property_count.saturating_sub(1);
        }
        Ok(removed)
    }

    pub(in crate::runtime::object) fn sparse_key(&self, index: ArrayIndex) -> Option<PropertyKey> {
        self.sparse_keys.get(&index).copied()
    }

    pub(in crate::runtime::object) fn insert_sparse_key(
        &mut self,
        index: ArrayIndex,
        key: PropertyKey,
    ) {
        self.sparse_keys.insert(index, key);
    }

    pub(in crate::runtime::object) fn remove_sparse_key(
        &mut self,
        index: ArrayIndex,
    ) -> Option<PropertyKey> {
        self.sparse_keys.remove(&index)
    }

    pub(in crate::runtime::object) fn sparse_keys(
        &self,
    ) -> impl Iterator<Item = (&ArrayIndex, &PropertyKey)> {
        self.sparse_keys.iter()
    }

    pub(in crate::runtime::object) fn has_sparse_keys(&self) -> bool {
        !self.sparse_keys.is_empty()
    }

    pub(in crate::runtime::object) fn visit_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        match &self.elements {
            ArrayElements::Packed(elements) => {
                for property in elements {
                    property.visit_strong_edges(VmObjectEdgeKind::Property, visitor)?;
                }
            }
            ArrayElements::Holey(elements) => {
                for property in elements.iter().flatten() {
                    property.visit_strong_edges(VmObjectEdgeKind::Property, visitor)?;
                }
            }
        }
        for key in self.sparse_keys.values() {
            visitor.visit(
                VmObjectEdgeKind::Property,
                StrongEdgeReference::PropertyKey(*key),
            )?;
        }
        Ok(())
    }

    fn checked_dense_len(position: usize) -> Result<usize> {
        position
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))
    }
}

impl Default for ArrayStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
enum ArrayElements {
    // Packed means the materialized dense prefix has no holes; callers must still
    // compare storage length with the JavaScript array length before full fast paths.
    Packed(Vec<ObjectProperty>),
    Holey(Vec<Option<ObjectProperty>>),
}
