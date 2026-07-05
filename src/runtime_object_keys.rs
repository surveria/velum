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
        object.extend_enumerable_keys(atoms, &mut keys)?;
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
            object.extend_enumerable_keys(atoms, keys)?;
            object.prototype
        };
        if let Some(prototype) = prototype {
            self.collect_keys(prototype, atoms, keys, visited)?;
        }
        Ok(())
    }
}

impl Object {
    const fn enumerable_key_count_hint(&self) -> usize {
        self.enumerable_property_count
    }

    fn extend_enumerable_keys(&self, atoms: &AtomTable, keys: &mut Vec<String>) -> Result<()> {
        if !self.has_enumerable_own_keys() {
            return Ok(());
        }
        if self.array_length.is_none() {
            self.extend_named_keys(atoms, keys, false)?;
            return Ok(());
        }

        self.extend_array_element_keys(keys);
        self.extend_sparse_array_element_keys(atoms, keys)?;
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
        for (index, property) in self.array_elements.iter().enumerate() {
            if property
                .as_ref()
                .is_some_and(super::ObjectProperty::is_enumerable)
            {
                push_unique_key(keys, index.to_string());
            }
        }
    }

    fn extend_sparse_array_element_keys(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        if self.sparse_array_keys.is_empty() {
            return Ok(());
        }
        let mut entries: Vec<(ArrayIndex, PropertyKey)> = Vec::new();
        for (index, key) in &self.sparse_array_keys {
            if self
                .named_property(*key)
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

fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if keys.iter().any(|existing| existing == &key) {
        return;
    }
    keys.push(key);
}
