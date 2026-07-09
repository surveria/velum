use crate::{
    error::{Error, Result},
    syntax::AccessorKind,
    value::{ObjectId, Value},
};

use super::{
    AccessorPropertyUpdate, ArrayLength, OBJECT_CONSTRUCTOR_PROPERTY, Object, ObjectHeap,
    ObjectPrimitiveValue, ObjectPropertyInit, ObjectPropertyValue, ObjectStructureSnapshot,
    PROTOTYPE_PROPERTY, PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyLookup,
    PropertyUpdate, RegExpValue, ShapeTable,
};

impl ObjectHeap {
    pub(crate) fn own_data_property_value(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let Some(key) = property.key() else {
            return Ok(None);
        };
        let object = self.object(id)?;
        if object.array_length.is_some() {
            return Ok(None);
        }
        let Some(property) = object.named_property(&self.shapes, key)? else {
            return Ok(None);
        };
        Ok(property.data_value_ref().cloned())
    }

    pub(crate) fn set_own_data_property_value(
        &mut self,
        id: ObjectId,
        property: PropertyLookup<'_>,
        value: Value,
    ) -> Result<bool> {
        let Some(key) = property.key() else {
            return Ok(false);
        };
        let object = self.object(id)?;
        if object.array_length.is_some() {
            return Ok(false);
        }
        let Some(property) = object.named_property(&self.shapes, key)? else {
            return Ok(false);
        };
        if property.data_value_ref().is_none() || !property.is_writable() {
            return Ok(false);
        }
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.named_property_mut(shapes, key)?.set_value(value);
        Ok(true)
    }

    pub fn create(
        &mut self,
        properties: Vec<ObjectPropertyInit<'_>>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        let mut literal_prototype = None;
        for property in properties {
            let uses_literal_prototype = property.uses_literal_prototype();
            let accessor = property.accessor_kind();
            let ObjectPropertyInit {
                key, name, value, ..
            } = property;
            if uses_literal_prototype && name == PROTOTYPE_PROPERTY {
                if let Some(prototype) = Object::literal_prototype(&value) {
                    literal_prototype = Some(prototype);
                }
            } else if let Some(kind) = accessor {
                let (get, set) = match kind {
                    AccessorKind::Getter => (Some(value), None),
                    AccessorKind::Setter => (None, Some(value)),
                };
                let update = PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    get,
                    set,
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                ));
                object.define_property(key, name, update, &mut self.shapes, max_properties)?;
            } else {
                object.set(key, name, value, &mut self.shapes, max_properties)?;
            }
        }
        object.prototype = match literal_prototype {
            Some(prototype) => prototype.into_object_id(),
            None => Some(self.object_prototype_id(constructor_key, max_objects, max_properties)?),
        };

        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
    }

    pub fn create_array(
        &mut self,
        elements: Vec<Value>,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let element_count = elements.len();
        self.create_array_from_iter(
            elements,
            element_count,
            prototype,
            max_objects,
            max_properties,
        )
    }

    pub(crate) fn create_array_from_iter(
        &mut self,
        elements: impl IntoIterator<Item = Value>,
        element_count: usize,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let length = ArrayLength::from_usize(element_count)?;
        let mut object = Object::array(length);
        object.prototype = Some(prototype);
        object.append_packed_default_value_iter(elements, element_count, max_properties)?;

        self.push_object(object, max_objects).map(Value::Object)
    }

    pub fn create_array_with_length(
        &mut self,
        length: usize,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let length = ArrayLength::from_usize(length)?;
        let mut object = Object::array(length);
        object.prototype = Some(prototype);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub fn create_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        self.create_with_prototype_id(prototype, constructor_key, max_objects, max_properties)
            .map(Value::Object)
    }

    pub(in crate::runtime) fn create_boxed_primitive(
        &mut self,
        value: ObjectPrimitiveValue,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let mut object = Object::boxed_primitive(value);
        object.prototype = Some(prototype);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn create_with_prototype_id(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(
            prototype,
            constructor_key,
            max_objects,
            max_properties,
        )?;
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }

    pub(crate) fn create_with_exact_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        max_objects: usize,
    ) -> Result<Value> {
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
    }

    pub(crate) fn create_regexp(
        &mut self,
        value: RegExpValue,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.regexp_value = Some(value);
        self.push_object(object, max_objects)
    }

    pub(crate) fn mark_raw_json(&mut self, id: ObjectId) -> Result<()> {
        self.object_mut(id)?.is_raw_json = true;
        Ok(())
    }

    pub(crate) fn is_raw_json(&self, id: ObjectId) -> Result<bool> {
        Ok(self.object(id)?.is_raw_json)
    }

    pub(crate) fn create_with_prototype_property(
        &mut self,
        prototype: Option<ObjectId>,
        property: ObjectPropertyInit<'_>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(
            prototype,
            constructor_key,
            max_objects,
            max_properties,
        )?;
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;
        object.define(
            property.key,
            property.name,
            property.value,
            property.enumerable,
            &mut self.shapes,
            max_properties,
        )?;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }

    fn resolve_default_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Option<ObjectId>> {
        if prototype.is_some() {
            return Ok(prototype);
        }
        self.object_prototype_id(constructor_key, max_objects, max_properties)
            .map(Some)
    }

    pub(crate) fn object_prototype_id(
        &mut self,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        if let Some(id) = self.object_prototype {
            return Ok(id);
        }
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.define(
            constructor_key,
            OBJECT_CONSTRUCTOR_PROPERTY,
            Value::String("Object".to_owned()),
            PropertyEnumerable::No,
            &mut self.shapes,
            max_properties,
        )?;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        self.object_prototype = Some(id);
        Ok(id)
    }

    pub(crate) fn array_prototype_id_with_constructor(
        &mut self,
        constructor: Value,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = if let Some(id) = self.array_prototype {
            id
        } else {
            let object_prototype =
                self.object_prototype_id(constructor_key, max_objects, max_properties)?;
            if self.objects.len() >= max_objects {
                return Err(Error::limit(format!("object count exceeded {max_objects}")));
            }

            let mut object = Object::ordinary();
            object.prototype = Some(object_prototype);
            let id = ObjectId::new(self.objects.len());
            self.objects.push(object);
            self.array_prototype = Some(id);
            id
        };

        self.define_non_enumerable(
            prototype,
            constructor_key,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
            max_properties,
        )?;
        Ok(prototype)
    }

    pub(crate) fn existing_array_prototype_id(&self) -> Result<ObjectId> {
        self.array_prototype
            .ok_or_else(|| Error::runtime("Array prototype is not initialized"))
    }

    pub(crate) fn define_non_enumerable(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.define(
            property,
            property_name,
            value,
            PropertyEnumerable::No,
            shapes,
            max_properties,
        )?;
        self.bump_if_structure_changed(id, before)
    }

    pub fn get(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<ObjectPropertyValue> {
        self.get_in_chain(id, property)
    }

    pub(in crate::runtime) fn primitive_value(
        &self,
        id: ObjectId,
    ) -> Result<Option<&ObjectPrimitiveValue>> {
        Ok(self.object(id)?.primitive_value.as_ref())
    }

    pub(in crate::runtime) fn regexp_value(&self, id: ObjectId) -> Result<Option<&RegExpValue>> {
        Ok(self.object(id)?.regexp_value.as_ref())
    }

    pub fn has(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.has_in_chain(id, property)
    }

    pub fn set(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let lookup = PropertyLookup::from_key(property_name, property);
        if property_name == PROTOTYPE_PROPERTY && !self.object(id)?.has_own(lookup, &self.shapes)? {
            return self.set_prototype(id, &value);
        }
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.set(property, property_name, value, shapes, max_properties)?;
        self.bump_if_structure_changed(id, before)
    }

    pub fn delete(&mut self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        if property.name() == PROTOTYPE_PROPERTY {
            if self.object(id)?.has_own(property, &self.shapes)? {
                let before = self.object(id)?.structure_snapshot();
                let (object, shapes) = self.object_mut_with_shapes(id)?;
                let deleted = object.delete(property, shapes)?;
                self.bump_if_structure_changed(id, before)?;
                return Ok(deleted);
            }
            return Ok(true);
        }
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        let deleted = object.delete(property, shapes)?;
        self.bump_if_structure_changed(id, before)?;
        Ok(deleted)
    }

    fn get_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<ObjectPropertyValue> {
        self.prototype_get_in_chain(id, property)
    }

    fn has_in_chain(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.prototype_has_in_chain(id, property)
    }

    fn set_prototype(&mut self, id: ObjectId, value: &Value) -> Result<()> {
        self.set_prototype_value(id, value)
    }

    pub(super) fn object(&self, id: ObjectId) -> Result<&Object> {
        self.objects
            .get(id.index())
            .ok_or_else(|| Error::runtime("object id is not defined"))
    }

    pub(super) fn object_mut(&mut self, id: ObjectId) -> Result<&mut Object> {
        self.objects
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("object id is not defined"))
    }

    pub(super) fn object_mut_with_shapes(
        &mut self,
        id: ObjectId,
    ) -> Result<(&mut Object, &mut ShapeTable)> {
        let object = self
            .objects
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("object id is not defined"))?;
        Ok((object, &mut self.shapes))
    }

    pub(crate) const fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    pub(super) fn bump_if_structure_changed(
        &mut self,
        id: ObjectId,
        before: ObjectStructureSnapshot,
    ) -> Result<()> {
        if self.object(id)?.structure_snapshot() == before {
            return Ok(());
        }
        self.bump_prototype_lookup_version()
    }

    pub(super) fn push_object(&mut self, object: Object, max_objects: usize) -> Result<ObjectId> {
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }
}
