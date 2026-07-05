use crate::{
    error::{Error, Result},
    value::ObjectId,
};

use super::{ArrayIndex, Object, ObjectHeap};

impl ObjectHeap {
    pub fn keys(&self, id: ObjectId) -> Result<Vec<String>> {
        let object = self.object(id)?;
        let mut keys = Vec::with_capacity(object.enumerable_key_count_hint());
        let mut visited = Vec::new();
        self.collect_keys(id, &mut keys, &mut visited)?;
        Ok(keys)
    }

    fn collect_keys(
        &self,
        id: ObjectId,
        keys: &mut Vec<String>,
        visited: &mut Vec<ObjectId>,
    ) -> Result<()> {
        if visited.contains(&id) {
            return Err(Error::runtime("prototype cycle detected"));
        }
        visited.push(id);
        let prototype = {
            let object = self.object(id)?;
            object.extend_enumerable_keys(keys);
            object.prototype
        };
        if let Some(prototype) = prototype {
            self.collect_keys(prototype, keys, visited)?;
        }
        Ok(())
    }
}

impl Object {
    const fn enumerable_key_count_hint(&self) -> usize {
        self.enumerable_property_count
    }

    fn extend_enumerable_keys(&self, keys: &mut Vec<String>) {
        if !self.has_enumerable_own_keys() {
            return;
        }
        if self.array_length.is_none() {
            self.extend_named_keys(keys, false);
            return;
        }

        self.extend_array_element_keys(keys);
        self.extend_sparse_array_element_keys(keys);
        self.extend_named_keys(keys, true);
    }

    fn extend_named_keys(&self, keys: &mut Vec<String>, skip_array_indices: bool) {
        for key in &self.property_order {
            if skip_array_indices && ArrayIndex::parse(key).is_some() {
                continue;
            }
            if self
                .properties
                .get(key)
                .is_some_and(super::ObjectProperty::is_enumerable)
            {
                push_unique_key(keys, key.clone());
            }
        }
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

    fn extend_sparse_array_element_keys(&self, keys: &mut Vec<String>) {
        if self.property_order.is_empty() {
            return;
        }
        let mut entries: Vec<(ArrayIndex, String)> = self
            .property_order
            .iter()
            .filter_map(|key| {
                let index = ArrayIndex::parse(key)?;
                self.properties
                    .get(key)
                    .filter(|property| property.is_enumerable())
                    .map(|_| (index, key.clone()))
            })
            .collect();
        entries.sort_by_key(|(index, _)| *index);
        for (_, key) in entries {
            push_unique_key(keys, key);
        }
    }
}

fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if keys.iter().any(|existing| existing == &key) {
        return;
    }
    keys.push(key);
}
