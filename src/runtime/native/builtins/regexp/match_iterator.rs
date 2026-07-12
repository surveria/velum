use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, collections::CollectionIteratorId},
    value::Value,
};

use super::{
    REGEXP_FLAGS_PROPERTY, REGEXP_LAST_INDEX_PROPERTY, REGEXP_STRING_ITERATOR_TAG,
    match_search::advance_match_index,
};

impl Context {
    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_match_all(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(receiver)?.is_none() {
            return Err(Error::type_error(
                "RegExp.prototype[Symbol.matchAll] requires an object receiver",
            ));
        }
        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        let input_value = self.heap_utf16_string_value(&input)?;
        let constructor = self.regexp_species_constructor(receiver)?;
        let flags_value = self.get_named(receiver, REGEXP_FLAGS_PROPERTY)?;
        let flags = self.to_string(&flags_value)?;
        let flags_value = self.heap_string_value(&flags)?;
        let matcher = self.semantic_construct(
            &constructor,
            &[receiver.clone(), flags_value],
            constructor.clone(),
        )?;
        let last_index = self.get_named(receiver, REGEXP_LAST_INDEX_PROPERTY)?;
        let last_index = Self::length_to_usize(
            self.to_length(&last_index)?,
            "RegExp matchAll lastIndex exceeded supported range",
        )?;
        let last_index = Self::usize_to_number(
            last_index,
            "RegExp matchAll lastIndex exceeded numeric range",
        )?;
        self.set_regexp_last_index_value(&matcher, Value::Number(last_index))?;
        let iterator = self.create_regexp_string_iterator(
            matcher,
            input_value,
            flags.contains('g'),
            flags.contains('u') || flags.contains('v'),
        )?;
        self.create_tagged_iterator_state_object(iterator, REGEXP_STRING_ITERATOR_TAG)
    }

    pub(in crate::runtime) fn regexp_string_iterator_step(
        &mut self,
        iterator: CollectionIteratorId,
    ) -> Result<Option<Value>> {
        let state = self.regexp_string_iterator_state(iterator)?.clone();
        if state.done {
            return Ok(None);
        }
        let input = self.to_utf16_string(&state.input)?;
        let result = self.regexp_exec_abstract(&state.matcher, &state.input, &input)?;
        let Some(result) = result else {
            self.finish_regexp_string_iterator(iterator)?;
            return Ok(None);
        };
        if !state.global {
            self.finish_regexp_string_iterator(iterator)?;
            return Ok(Some(result));
        }
        let match_value = self.get_named(&result, "0")?;
        let match_units = self.to_utf16_string(&match_value)?;
        if match_units.is_empty() {
            let last_index = self.get_named(&state.matcher, REGEXP_LAST_INDEX_PROPERTY)?;
            let last_index = Self::length_to_usize(
                self.to_length(&last_index)?,
                "RegExp string iterator lastIndex exceeded supported range",
            )?;
            let next = advance_match_index(&input, last_index, state.unicode)?;
            self.set_regexp_last_index(&state.matcher, next)?;
        }
        Ok(Some(result))
    }
}
