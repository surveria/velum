#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::object::TypedArrayView,
    storage::{atom::AtomTable, symbol::SymbolTable},
    value::ObjectId,
};

use super::{
    ARRAY_LENGTH_PROPERTY, ArrayIndex, Object, ObjectHeap, ObjectProperty, PropertyKey, ShapeTable,
};

impl ObjectHeap {
    pub fn keys(&self, id: ObjectId, atoms: &AtomTable) -> Result<Vec<String>> {
        let object = self.object(id)?;
        let mut keys = Vec::with_capacity(object.enumerable_key_count_hint());
        let mut visited_objects = Vec::new();
        let mut visited_names = Vec::new();
        self.collect_keys(
            id,
            atoms,
            &mut keys,
            &mut visited_objects,
            &mut visited_names,
        )?;
        Ok(keys)
    }

    pub(crate) fn own_property_names(
        &self,
        id: ObjectId,
        atoms: &AtomTable,
    ) -> Result<Vec<String>> {
        let object = self.object(id)?;
        let mut keys = Vec::with_capacity(object.property_count());
        object.extend_own_property_names(atoms, &mut keys)?;
        Ok(keys)
    }

    pub(crate) fn own_property_symbols(
        &self,
        id: ObjectId,
        symbols: &SymbolTable,
    ) -> Result<Vec<crate::storage::symbol::JsSymbol>> {
        let object = self.object(id)?;
        let mut keys = Vec::new();
        object.extend_own_property_symbols(symbols, &mut keys)?;
        Ok(keys)
    }

    fn collect_keys(
        &self,
        id: ObjectId,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
        visited_objects: &mut Vec<ObjectId>,
        visited_names: &mut Vec<String>,
    ) -> Result<()> {
        if visited_objects.contains(&id) {
            return Err(Error::runtime("prototype cycle detected"));
        }
        visited_objects.push(id);
        let prototype = {
            let object = self.object(id)?;
            let mut own_names = Vec::new();
            object.extend_own_property_names(atoms, &mut own_names)?;
            let mut newly_visited = Vec::new();
            for name in own_names {
                if !visited_names.contains(&name) {
                    visited_names.push(name.clone());
                    newly_visited.push(name);
                }
            }
            let mut enumerable_names = Vec::new();
            object.extend_enumerable_keys(atoms, &self.shapes, &mut enumerable_names)?;
            for name in enumerable_names {
                if newly_visited.contains(&name) {
                    keys.push(name);
                }
            }
            object.ordinary_prototype_id()
        };
        if let Some(prototype) = prototype {
            self.collect_keys(prototype, atoms, keys, visited_objects, visited_names)?;
        }
        Ok(())
    }
}

impl Object {
    fn enumerable_key_count_hint(&self) -> usize {
        self.enumerable_property_count
            .saturating_add(self.virtual_string_key_count())
            .saturating_add(self.typed_array.as_ref().map_or(0, TypedArrayView::length))
    }

    fn extend_enumerable_keys(
        &self,
        atoms: &AtomTable,
        shapes: &ShapeTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        if !self.has_enumerable_own_keys() {
            return Ok(());
        }
        self.extend_virtual_string_keys(keys)?;
        self.extend_typed_array_index_names(keys);
        if self.array_length.is_none() {
            self.extend_named_array_index_keys(atoms, keys)?;
            self.extend_named_keys(atoms, keys, true)?;
            return Ok(());
        }

        self.extend_array_element_keys(keys);
        self.extend_sparse_array_element_keys(atoms, shapes, keys)?;
        self.extend_named_keys(atoms, keys, true)
    }

    fn extend_own_property_names(&self, atoms: &AtomTable, keys: &mut Vec<String>) -> Result<()> {
        self.extend_virtual_string_keys(keys)?;
        self.extend_typed_array_index_names(keys);
        if self.array_length.is_some() {
            self.extend_array_element_names(keys);
            self.extend_sparse_array_element_names(atoms, keys)?;
            push_unique_key(keys, ARRAY_LENGTH_PROPERTY.to_owned());
        } else {
            self.extend_named_array_index_names(atoms, keys)?;
        }
        self.extend_named_property_names(atoms, keys, true)?;
        Ok(())
    }

    fn extend_typed_array_index_names(&self, keys: &mut Vec<String>) {
        let Some(view) = self.typed_array.as_ref() else {
            return;
        };
        for index in 0..view.length() {
            push_unique_key(keys, index.to_string());
        }
    }

    fn extend_own_property_symbols(
        &self,
        symbols: &SymbolTable,
        keys: &mut Vec<crate::storage::symbol::JsSymbol>,
    ) -> Result<()> {
        for named_property in self.named_properties() {
            let Some(symbol) = named_property.key().symbol_id() else {
                continue;
            };
            keys.push(symbols.get(symbol)?.clone());
        }
        Ok(())
    }

    fn extend_named_array_index_keys(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        let mut entries = Vec::new();
        for named_property in self.named_properties() {
            let key = named_property.key();
            let Some(atom) = key.atom() else {
                continue;
            };
            let name = atoms.name(atom)?;
            let Some(index) = ArrayIndex::parse(name) else {
                continue;
            };
            if named_property.property().is_enumerable() {
                entries.push((index, name.to_owned()));
            }
        }
        entries.sort_by_key(|(index, _)| *index);
        for (_, name) in entries {
            push_unique_key(keys, name);
        }
        Ok(())
    }

    fn extend_named_array_index_names(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        let mut entries = Vec::new();
        for named_property in self.named_properties() {
            let key = named_property.key();
            let Some(atom) = key.atom() else {
                continue;
            };
            let name = atoms.name(atom)?;
            let Some(index) = ArrayIndex::parse(name) else {
                continue;
            };
            entries.push((index, name.to_owned()));
        }
        entries.sort_by_key(|(index, _)| *index);
        for (_, name) in entries {
            push_unique_key(keys, name);
        }
        Ok(())
    }

    fn extend_named_keys(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
        skip_array_indices: bool,
    ) -> Result<()> {
        for named_property in self.named_properties() {
            let key = named_property.key();
            let Some(atom) = key.atom() else {
                continue;
            };
            let name = atoms.name(atom)?;
            if skip_array_indices && ArrayIndex::parse(name).is_some() {
                continue;
            }
            if named_property.property().is_enumerable() {
                push_unique_key(keys, name.to_owned());
            }
        }
        Ok(())
    }

    fn extend_named_property_names(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
        skip_array_indices: bool,
    ) -> Result<()> {
        for named_property in self.named_properties() {
            let key = named_property.key();
            let Some(atom) = key.atom() else {
                continue;
            };
            let name = atoms.name(atom)?;
            if skip_array_indices && ArrayIndex::parse(name).is_some() {
                continue;
            }
            push_unique_key(keys, name.to_owned());
        }
        Ok(())
    }

    fn extend_array_element_keys(&self, keys: &mut Vec<String>) {
        for index in 0..self.array_storage.dense_len() {
            if self
                .array_storage
                .dense_property_at_position(index)
                .is_some_and(ObjectProperty::is_enumerable)
            {
                push_unique_key(keys, index.to_string());
            }
        }
    }

    fn extend_array_element_names(&self, keys: &mut Vec<String>) {
        for index in 0..self.array_storage.dense_len() {
            if self
                .array_storage
                .dense_property_at_position(index)
                .is_some()
            {
                push_unique_key(keys, index.to_string());
            }
        }
    }

    fn extend_sparse_array_element_keys(
        &self,
        atoms: &AtomTable,
        shapes: &ShapeTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        if !self.array_storage.has_sparse_keys() {
            return Ok(());
        }
        let mut entries: Vec<(ArrayIndex, PropertyKey)> = Vec::new();
        for (index, key) in self.array_storage.sparse_keys() {
            if self
                .named_property(shapes, *key)?
                .is_some_and(ObjectProperty::is_enumerable)
            {
                entries.push((*index, *key));
            }
        }
        entries.sort_by_key(|(index, _)| *index);
        for (_, key) in entries {
            let Some(atom) = key.atom() else {
                continue;
            };
            push_unique_key(keys, atoms.name(atom)?.to_owned());
        }
        Ok(())
    }

    fn extend_sparse_array_element_names(
        &self,
        atoms: &AtomTable,
        keys: &mut Vec<String>,
    ) -> Result<()> {
        for (_, key) in self.array_storage.sparse_keys() {
            let Some(atom) = key.atom() else {
                continue;
            };
            push_unique_key(keys, atoms.name(atom)?.to_owned());
        }
        Ok(())
    }
}

pub(in crate::runtime::object) fn push_unique_key(keys: &mut Vec<String>, key: String) {
    if keys.iter().any(|existing| existing == &key) {
        return;
    }
    keys.push(key);
}
