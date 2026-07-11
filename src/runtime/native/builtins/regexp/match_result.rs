use std::ops::Range;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
            PropertyWritable,
        },
    },
    value::Value,
};

use super::engine::{RegExpMatch, RegExpSpan, regexp_index_usize_to_number};

impl Context {
    pub(super) fn regexp_match_array(
        &mut self,
        input: &str,
        matched: &RegExpMatch,
        has_indices: bool,
    ) -> Result<Value> {
        let matched_text = input
            .get(matched.span.bytes.clone())
            .ok_or_else(|| Error::runtime("RegExp match span is not a string boundary"))?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let capture_count = matched
            .captures
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp capture count exceeded supported range"))?;
        let mut values = Vec::with_capacity(capture_count);
        values.push(self.heap_string_value(matched_text)?);
        for capture in &matched.captures {
            values.push(self.regexp_capture_value(input, capture.as_ref())?);
        }
        let array = self.objects.create_array(
            values,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(id) = array else {
            return Err(Error::runtime("RegExp match result is not an array object"));
        };
        self.define_regexp_data_property(
            id,
            "index",
            Value::Number(regexp_index_usize_to_number(matched.span.code_units.start)?),
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        )?;
        let input_value = self.heap_string_value(input)?;
        self.define_regexp_data_property(
            id,
            "input",
            input_value,
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        )?;
        let groups = self.regexp_named_capture_groups(input, &matched.named_captures)?;
        self.define_regexp_data_property(
            id,
            "groups",
            groups,
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        )?;
        if has_indices {
            let indices = self.regexp_match_indices(matched)?;
            self.define_regexp_data_property(
                id,
                "indices",
                indices,
                PropertyWritable::Yes,
                PropertyEnumerable::Yes,
                PropertyConfigurable::Yes,
            )?;
        }
        Ok(Value::Object(id))
    }

    fn regexp_capture_value(&mut self, input: &str, span: Option<&RegExpSpan>) -> Result<Value> {
        let Some(span) = span else {
            return Ok(Value::Undefined);
        };
        let text = input
            .get(span.bytes.clone())
            .ok_or_else(|| Error::runtime("RegExp capture span is not a string boundary"))?;
        self.heap_string_value(text)
    }

    fn regexp_named_capture_groups(
        &mut self,
        input: &str,
        captures: &[(String, Option<RegExpSpan>)],
    ) -> Result<Value> {
        if captures.is_empty() {
            return Ok(Value::Undefined);
        }
        let groups = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?;
        let Value::Object(groups_id) = groups else {
            return Err(Error::runtime("RegExp groups value is not an object"));
        };
        for (name, range) in captures {
            let value = self.regexp_capture_value(input, range.as_ref())?;
            let key = self.intern_property_key(name)?;
            self.objects.define_property(
                groups_id,
                key,
                name,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(value),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(Value::Object(groups_id))
    }

    fn regexp_match_indices(&mut self, matched: &RegExpMatch) -> Result<Value> {
        let count = matched
            .captures
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp indices count exceeded supported range"))?;
        let mut values = Vec::with_capacity(count);
        values.push(self.regexp_index_pair(matched.span.code_units.clone())?);
        for capture in &matched.captures {
            values.push(if let Some(span) = capture {
                self.regexp_index_pair(span.code_units.clone())?
            } else {
                Value::Undefined
            });
        }
        let prototype = self.objects.existing_array_prototype_id()?;
        let array = self.objects.create_array(
            values,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(id) = array else {
            return Err(Error::runtime(
                "RegExp indices result is not an array object",
            ));
        };
        let groups = self.regexp_named_capture_indices_groups(&matched.named_captures)?;
        self.define_regexp_data_property(
            id,
            "groups",
            groups,
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        )?;
        Ok(Value::Object(id))
    }

    fn regexp_named_capture_indices_groups(
        &mut self,
        captures: &[(String, Option<RegExpSpan>)],
    ) -> Result<Value> {
        if captures.is_empty() {
            return Ok(Value::Undefined);
        }
        let groups = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?;
        let Value::Object(groups_id) = groups else {
            return Err(Error::runtime(
                "RegExp indices groups value is not an object",
            ));
        };
        for (name, span) in captures {
            let value = if let Some(span) = span {
                self.regexp_index_pair(span.code_units.clone())?
            } else {
                Value::Undefined
            };
            let key = self.intern_property_key(name)?;
            self.objects.define_property(
                groups_id,
                key,
                name,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(value),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(Value::Object(groups_id))
    }

    fn regexp_index_pair(&mut self, range: Range<usize>) -> Result<Value> {
        let values = vec![
            Value::Number(regexp_index_usize_to_number(range.start)?),
            Value::Number(regexp_index_usize_to_number(range.end)?),
        ];
        let prototype = self.objects.existing_array_prototype_id()?;
        self.objects.create_array(
            values,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
