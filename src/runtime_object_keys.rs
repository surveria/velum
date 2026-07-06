use crate::{
    atom::AtomTable,
    error::{Error, Result},
    value::ObjectId,
};

use super::{ArrayIndex, Object, ObjectHeap, PropertyKey};

impl ObjectHeap {
    pub fn keys(&self, id: ObjectId, atoms: &AtomTable) -> Result<Vec<String>> {
        let object = self.object(id)?;
        let mut keys = Vec::with_capacity(object.enumerable_key_count_hint());
        let mut visited = Vec::new();
        self.collect_keys(id, atoms, &mut keys, &mut visited)?;
        Ok(keys)
    }

    pub(crate) fn own_keys(&self, id: ObjectId, atoms: &AtomTable) -> Result<Vec<String>> {
        let object = self.object(id)?;
        let mut keys = Vec::with_capacity(object.enumerable_key_count_hint());
        object.extend_enumerable_keys(atoms, &self.shapes, &mut keys)?;
        Ok(keys)
    }

    fn collect_keys(
        &self,
        id: ObjectId,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
        visited: &mut Vec<ObjectId>,
    ) -> Result<()> {
        if visited.contains(&id) {
            return Err(Error::runtime("prototype cycle detected"));
        }
        visited.push(id);
        let prototype = {
            let object = self.object(id)?;
            object.extend_enumerable_keys(atoms, &self.shapes, keys)?;
            object.prototype
        };
        if let Some(prototype) = prototype {
            self.collect_keys(prototype, atoms, keys, visited)?;
        }
        Ok(())
    }
}

impl Object {
    fn enumerable_key_count_hint(&self) -> usize {
        self.enumerable_property_count
            .saturating_add(self.virtual_string_key_count())
    }

    fn extend_enumerable_keys(
        &self,
        atoms: &AtomTable,
        shapes: &super::ShapeTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        if !self.has_enumerable_own_keys() {
            return Ok(());
        }
        self.extend_virtual_string_keys(keys)?;
        if self.array_length.is_none() {
            self.extend_named_keys(atoms, keys, false)?;
            return Ok(());
        }

        self.extend_array_element_keys(keys);
        self.extend_sparse_array_element_keys(atoms, shapes, keys)?;
        self.extend_named_keys(atoms, keys, true)
    }

    fn extend_named_keys(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
        skip_array_indices: bool,
    ) -> Result<()> {
        for named_property in self.named_properties() {
            let key = named_property.key();
            let name = atoms.name(key.atom())?;
            if skip_array_indices && ArrayIndex::parse(name).is_some() {
                continue;
            }
            if named_property.property().is_enumerable() {
                push_unique_key(keys, name.to_owned());
            }
        }
        Ok(())
    }

    fn extend_array_element_keys(&self, keys: &mut Vec<String>) {
        for index in 0..self.array_storage.dense_len() {
            if self
                .array_storage
                .dense_property_at_position(index)
                .is_some_and(super::ObjectProperty::is_enumerable)
            {
                push_unique_key(keys, index.to_string());
            }
        }
    }

    fn extend_sparse_array_element_keys(
        &self,
        atoms: &AtomTable,
        shapes: &super::ShapeTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        if !self.array_storage.has_sparse_keys() {
            return Ok(());
        }
        let mut entries: Vec<(ArrayIndex, PropertyKey)> = Vec::new();
        for (index, key) in self.array_storage.sparse_keys() {
            if self
                .named_property(shapes, *key)?
                .is_some_and(super::ObjectProperty::is_enumerable)
            {
                entries.push((*index, *key));
            }
        }
        entries.sort_by_key(|(index, _)| *index);
        for (_, key) in entries {
            push_unique_key(keys, atoms.name(key.atom())?.to_owned());
        }
        Ok(())
    }
}

pub(super) fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if keys.iter().any(|existing| existing == &key) {
        return;
    }
    keys.push(key);
}
