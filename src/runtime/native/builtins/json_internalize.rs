use crate::{
    error::Result,
    runtime::{
        Context,
        abstract_operations::same_value,
        object::{
            DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, OwnPropertyDescriptor,
            PropertyConfigurable, PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        transient_roots::TransientRootScope,
    },
    value::Value,
};

use super::json_parse::ParsedJson;

const JSON_SOURCE_NAME: &str = "source";

#[derive(Debug)]
pub(super) struct JsonParseRecord {
    original: Value,
    source: Option<Vec<u16>>,
    children: JsonParseChildren,
}

#[derive(Debug)]
enum JsonParseChildren {
    None,
    Array(Vec<JsonParseRecord>),
    Object(Vec<(String, JsonParseRecord)>),
}

impl JsonParseRecord {
    fn scalar(original: Value, source: Vec<u16>) -> Self {
        Self {
            original,
            source: Some(source),
            children: JsonParseChildren::None,
        }
    }

    fn array(original: Value, children: Vec<Self>) -> Self {
        Self {
            original,
            source: None,
            children: JsonParseChildren::Array(children),
        }
    }

    fn object(original: Value, children: Vec<(String, Self)>) -> Self {
        Self {
            original,
            source: None,
            children: JsonParseChildren::Object(children),
        }
    }

    fn child(&self, key: &str) -> Option<&Self> {
        match &self.children {
            JsonParseChildren::None => None,
            JsonParseChildren::Array(children) => key
                .parse::<usize>()
                .ok()
                .and_then(|index| children.get(index)),
            JsonParseChildren::Object(children) => children
                .iter()
                .rev()
                .find_map(|(name, record)| (name == key).then_some(record)),
        }
    }

    pub(super) fn add_original_object_roots(&self, roots: &TransientRootScope) -> Result<()> {
        if matches!(self.original, Value::Object(_)) {
            roots.add_values(std::iter::once(&self.original))?;
        }
        match &self.children {
            JsonParseChildren::None => Ok(()),
            JsonParseChildren::Array(children) => {
                for child in children {
                    child.add_original_object_roots(roots)?;
                }
                Ok(())
            }
            JsonParseChildren::Object(children) => {
                for (_, child) in children {
                    child.add_original_object_roots(roots)?;
                }
                Ok(())
            }
        }
    }
}

impl Context {
    pub(super) fn value_and_record_from_json(
        &mut self,
        parsed: ParsedJson,
    ) -> Result<(Value, JsonParseRecord)> {
        match parsed {
            ParsedJson::Null { source } => {
                let value = Value::Null;
                Ok((value.clone(), JsonParseRecord::scalar(value, source)))
            }
            ParsedJson::Bool { value, source } => {
                let value = Value::Bool(value);
                Ok((value.clone(), JsonParseRecord::scalar(value, source)))
            }
            ParsedJson::Number { value, source } => {
                let value = Value::Number(value);
                Ok((value.clone(), JsonParseRecord::scalar(value, source)))
            }
            ParsedJson::String { value, source } => {
                let value = self.heap_utf16_string_value(&value)?;
                Ok((value.clone(), JsonParseRecord::scalar(value, source)))
            }
            ParsedJson::Array(values) => self.array_and_record_from_json(values),
            ParsedJson::Object(properties) => self.object_and_record_from_json(properties),
        }
    }

    fn array_and_record_from_json(
        &mut self,
        parsed: Vec<ParsedJson>,
    ) -> Result<(Value, JsonParseRecord)> {
        let roots = self
            .active_transient_root_scope(crate::runtime::roots::VmRootKind::TransientTemporary)?;
        let mut values = Vec::with_capacity(parsed.len());
        let mut records = Vec::with_capacity(parsed.len());
        for child in parsed {
            let (value, record) = self.value_and_record_from_json(child)?;
            roots.add_values(std::iter::once(&value))?;
            values.push(value);
            records.push(record);
        }
        let value = self.create_array_from_elements(values)?;
        Ok((value.clone(), JsonParseRecord::array(value, records)))
    }

    fn object_and_record_from_json(
        &mut self,
        parsed: Vec<(Vec<u16>, ParsedJson)>,
    ) -> Result<(Value, JsonParseRecord)> {
        let roots = self
            .active_transient_root_scope(crate::runtime::roots::VmRootKind::TransientTemporary)?;
        let mut names = Vec::with_capacity(parsed.len());
        let mut values = Vec::with_capacity(parsed.len());
        let mut records = Vec::with_capacity(parsed.len());
        for (key, child) in parsed {
            self.check_utf16_string_len(&key)?;
            let name = String::from_utf16_lossy(&key);
            self.check_string_len(&name)?;
            let property = self.intern_property_key(&name)?;
            let (value, record) = self.value_and_record_from_json(child)?;
            roots.add_values(std::iter::once(&value))?;
            records.push((name.clone(), record));
            names.push(name);
            values.push((property, value));
        }
        let properties = names
            .iter()
            .zip(values)
            .map(|(name, (property, value))| {
                ObjectPropertyInit::new_data(
                    property,
                    name.as_str(),
                    value,
                    PropertyEnumerable::Yes,
                )
            })
            .collect();
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        Ok((value.clone(), JsonParseRecord::object(value, records)))
    }

    pub(super) fn internalize_json_property(
        &mut self,
        holder: &Value,
        key: &str,
        reviver: &Value,
        record: Option<&JsonParseRecord>,
    ) -> Result<Value> {
        let value = self.get_named(holder, key)?;
        let matching_record = record.filter(|candidate| same_value(&candidate.original, &value));
        if matches!(value, Value::Object(_)) {
            self.internalize_json_children(&value, reviver, matching_record)?;
        }
        let key_value = self.heap_string_value(key)?;
        let context = self.create_json_reviver_context(
            matching_record.and_then(|candidate| candidate.source.as_deref()),
        )?;
        let args = [key_value, value, context];
        self.call_json_callback(reviver, holder.clone(), &args)
    }

    fn internalize_json_children(
        &mut self,
        holder: &Value,
        reviver: &Value,
        record: Option<&JsonParseRecord>,
    ) -> Result<()> {
        if self.semantic_is_array(holder)? {
            let length = self.array_like_length(holder)?;
            for index in 0..length {
                let key = index.to_string();
                self.internalize_json_child(
                    holder,
                    &key,
                    reviver,
                    record.and_then(|r| r.child(&key)),
                )?;
            }
            return Ok(());
        }
        for key in self.semantic_own_enumerable_string_keys(holder)? {
            self.internalize_json_child(holder, &key, reviver, record.and_then(|r| r.child(&key)))?;
        }
        Ok(())
    }

    fn internalize_json_child(
        &mut self,
        holder: &Value,
        key: &str,
        reviver: &Value,
        record: Option<&JsonParseRecord>,
    ) -> Result<()> {
        let value = self.internalize_json_property(holder, key, reviver, record)?;
        if matches!(value, Value::Undefined) {
            return self.delete_json_property(holder, key);
        }
        self.set_json_property(holder, key, value)
    }

    fn create_json_reviver_context(&mut self, source: Option<&[u16]>) -> Result<Value> {
        let properties = if let Some(source) = source {
            let value = self.heap_utf16_string_value(source)?;
            let key = self.intern_property_key(JSON_SOURCE_NAME)?;
            vec![ObjectPropertyInit::new_data(
                key,
                JSON_SOURCE_NAME,
                value,
                PropertyEnumerable::Yes,
            )]
        } else {
            Vec::new()
        };
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn delete_json_property(&mut self, holder: &Value, key: &str) -> Result<()> {
        let lookup = self.property_lookup(key);
        self.delete_property_value_with_lookup(holder, lookup)?;
        Ok(())
    }

    fn set_json_property(&mut self, holder: &Value, key: &str, value: Value) -> Result<()> {
        let descriptor = DataPropertyDescriptor::new(
            value.clone(),
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        );
        let descriptor_value =
            self.create_property_descriptor_object(&OwnPropertyDescriptor::Data(descriptor))?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(PropertyWritable::Yes),
            Some(PropertyEnumerable::Yes),
            Some(PropertyConfigurable::Yes),
        ));
        let mut property = DynamicPropertyKey::new(key.to_owned(), None);
        self.semantic_define_own_property_update_with_descriptor(
            holder,
            &mut property,
            update,
            &descriptor_value,
        )?;
        Ok(())
    }
}
