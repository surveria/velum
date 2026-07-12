use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable, RegExpValue,
        },
    },
    value::{ObjectId, Value},
};

mod compile;
mod engine;
mod match_result;
mod replace;
mod split;

use engine::{
    escaped_regexp_source_utf16, parse_regexp_flags, regexp_find_utf16,
    regexp_index_usize_to_number, regexp_test_utf16, validate_regexp_pattern_utf16,
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, REGEXP_NAME, REGEXP_PROTOTYPE_EXEC_NAME,
    REGEXP_PROTOTYPE_TEST_NAME, REGEXP_PROTOTYPE_TO_STRING_NAME,
};

const REGEXP_DOT_ALL_PROPERTY: &str = "dotAll";
const REGEXP_COMPILE_PROPERTY: &str = "compile";
const REGEXP_SOURCE_PROPERTY: &str = "source";
const REGEXP_FLAGS_PROPERTY: &str = "flags";
const REGEXP_GLOBAL_PROPERTY: &str = "global";
const REGEXP_HAS_INDICES_PROPERTY: &str = "hasIndices";
const REGEXP_IGNORE_CASE_PROPERTY: &str = "ignoreCase";
const REGEXP_LAST_INDEX_PROPERTY: &str = "lastIndex";
const REGEXP_MULTILINE_PROPERTY: &str = "multiline";
const REGEXP_RECEIVER_ERROR: &str = "RegExp method requires a RegExp receiver";
const REGEXP_STICKY_PROPERTY: &str = "sticky";
const REGEXP_UNICODE_PROPERTY: &str = "unicode";
const REGEXP_UNICODE_SETS_PROPERTY: &str = "unicodeSets";
const SYMBOL_MATCH_ALL_PROPERTY: &str = "matchAll";
const SYMBOL_MATCH_PROPERTY: &str = "match";
const SYMBOL_REPLACE_PROPERTY: &str = "replace";
const SYMBOL_SEARCH_PROPERTY: &str = "search";
const SYMBOL_SPLIT_PROPERTY: &str = "split";
const REGEXP_STRING_ITERATOR_TAG: &str = "RegExp String Iterator";
const ZERO_INDEX: f64 = 0.0;

impl Context {
    pub(in crate::runtime::native) fn regexp_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::RegExp) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.regexp_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::RegExp)?;
        self.push_native_function_with_id(id, NativeFunctionKind::RegExp, prototype, name)?;
        self.install_species_accessor(id)?;
        self.install_regexp_prototype_methods(prototype_id)?;
        self.install_regexp_prototype_symbol_methods(prototype_id)?;
        self.insert_global_builtin(REGEXP_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(crate) fn create_regexp_literal(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        self.create_regexp_object_from_text(pattern, flags)
    }

    pub(in crate::runtime::native) fn eval_regexp_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_regexp_constructor(args.as_slice(), RegExpCallMode::Call)
    }

    pub(in crate::runtime::native) fn eval_direct_regexp_constructor(
        &mut self,
        args: &[Value],
        mode: RegExpCallMode,
    ) -> Result<Value> {
        let flags_value = args.get(1);
        if let Some((pattern, flags)) = self.regexp_pattern_and_flags(args.first(), flags_value)? {
            if mode == RegExpCallMode::Call
                && flags_value.is_none_or(value_is_undefined)
                && let Some(value) = args.first()
            {
                return Ok(value.clone());
            }
            return self.create_regexp_object_from_utf16(&pattern, &flags);
        }
        let pattern = match args.first() {
            None | Some(Value::Undefined) => Vec::new(),
            Some(value) => self.to_utf16_string(value)?,
        };
        let flags = match flags_value {
            None | Some(Value::Undefined) => String::new(),
            Some(value) => self.to_string(value)?,
        };
        self.create_regexp_object_from_utf16(&pattern, &flags)
    }

    pub(in crate::runtime::native) fn construct_regexp_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_regexp_constructor(args.as_slice(), RegExpCallMode::Construct)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_exec(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        self.regexp_exec_code_units(this_value, &input)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_test(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = self.regexp_argument_utf16_or_undefined(args.as_slice().first())?;
        self.regexp_test_code_units(this_value, &input)
            .map(Value::Bool)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_regexp_extra_args(args.as_slice());
        let Value::Object(_) = this_value else {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        };
        let source_value = self.get_named(this_value, REGEXP_SOURCE_PROPERTY)?;
        let source = self.to_string(&source_value)?;
        let flags_value = self.get_named(this_value, REGEXP_FLAGS_PROPERTY)?;
        let flags = self.to_string(&flags_value)?;
        let capacity = source
            .len()
            .checked_add(flags.len())
            .and_then(|length| length.checked_add(2))
            .ok_or_else(|| Error::limit("RegExp.prototype.toString result length overflowed"))?;
        let mut text = String::with_capacity(capacity);
        text.push('/');
        text.push_str(&source);
        text.push('/');
        text.push_str(&flags);
        self.check_string_len(&text)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_match(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = self.regexp_argument_or_undefined(args.as_slice().first())?;
        let global = to_boolean(&self.get_named(this_value, REGEXP_GLOBAL_PROPERTY)?);
        if !global {
            return self.regexp_exec(this_value, &input);
        }
        let data = self.regexp_receiver_data(this_value)?;
        let flags = parse_regexp_flags(data.flags())?;
        self.set_regexp_last_index(this_value, 0)?;
        let mut matches = Vec::new();
        while let Some(matched) = self.regexp_match_text(this_value, &input)? {
            let is_empty = matched.is_empty();
            matches.push(matched);
            if matches.len() > self.limits.max_object_properties {
                return Err(Error::limit("RegExp match result exceeded array limit"));
            }
            if is_empty {
                let index = self.regexp_last_index(this_value, &input)?;
                let next =
                    advance_utf16_index(&input, index, flags.unicode() || flags.unicode_sets())?;
                self.set_regexp_last_index(this_value, next)?;
            }
        }
        if matches.is_empty() {
            return Ok(Value::Null);
        }
        self.regexp_string_array(matches)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_match_all(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = self.regexp_argument_or_undefined(args.as_slice().first())?;
        let matches = self.regexp_match_all_results(this_value, &input)?;
        self.create_tagged_collection_iterator_object(matches, REGEXP_STRING_ITERATOR_TAG)
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_symbol_search(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let input = self.regexp_argument_or_undefined(args.as_slice().first())?;
        let previous = self.get_named(this_value, REGEXP_LAST_INDEX_PROPERTY)?;
        self.set_regexp_last_index(this_value, 0)?;
        let result = self.regexp_exec(this_value, &input)?;
        self.set_regexp_last_index_value(this_value, previous)?;
        let Value::Object(id) = result else {
            return Ok(Value::Number(-1.0));
        };
        let index = self
            .get_named(&Value::Object(id), "index")?
            .as_number()
            .ok_or_else(|| Error::runtime("RegExp search result index is not numeric"))?;
        Ok(Value::Number(index))
    }

    fn create_regexp_object_from_text(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        let pattern = pattern.encode_utf16().collect::<Vec<_>>();
        self.create_regexp_object_from_utf16(&pattern, flags)
    }

    fn create_regexp_object_from_utf16(&mut self, pattern: &[u16], flags: &str) -> Result<Value> {
        self.charge_regexp_utf16_work(pattern, &[])?;
        let parsed_flags = parse_regexp_flags(flags)?;
        validate_regexp_pattern_utf16(pattern, parsed_flags)?;
        self.check_utf16_string_len(pattern)?;
        self.check_string_len(flags)?;
        let prototype = self.regexp_constructor_prototype()?;
        let id = self.objects.create_regexp(
            RegExpValue::new_utf16(pattern.to_vec(), flags.to_owned()),
            prototype,
            self.limits.max_objects,
        )?;
        self.define_regexp_data_property(
            id,
            REGEXP_LAST_INDEX_PROPERTY,
            Value::Number(ZERO_INDEX),
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        )?;
        Ok(Value::Object(id))
    }

    fn regexp_pattern_and_flags(
        &mut self,
        pattern_value: Option<&Value>,
        flags_value: Option<&Value>,
    ) -> Result<Option<(Vec<u16>, String)>> {
        let Some(Value::Object(id)) = pattern_value else {
            return Ok(None);
        };
        let Some(regexp) = self.objects.regexp_value(*id)?.cloned() else {
            return Ok(None);
        };
        let pattern = regexp.pattern_utf16().to_vec();
        let flags = if flags_value.is_none_or(value_is_undefined) {
            regexp.flags().to_owned()
        } else {
            let Some(value) = flags_value else {
                return Err(Error::runtime("RegExp flags value is unavailable"));
            };
            self.to_string(value)?
        };
        Ok(Some((pattern, flags)))
    }

    fn regexp_argument_or_undefined(&mut self, value: Option<&Value>) -> Result<String> {
        match value {
            Some(value) => self.to_string(value),
            None => self.to_string(&Value::Undefined),
        }
    }

    fn regexp_argument_utf16_or_undefined(&mut self, value: Option<&Value>) -> Result<Vec<u16>> {
        match value {
            Some(value) => self.to_utf16_string(value),
            None => self.to_utf16_string(&Value::Undefined),
        }
    }

    fn define_regexp_data_property(
        &mut self,
        id: ObjectId,
        name: &str,
        value: Value,
        writable: PropertyWritable,
        enumerable: PropertyEnumerable,
        configurable: PropertyConfigurable,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(writable),
            Some(enumerable),
            Some(configurable),
        ));
        self.objects
            .define_property(id, key, name, update, self.limits.max_object_properties)
    }

    fn regexp_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn regexp_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.regexp_constructor_value()? else {
            return Err(Error::runtime("RegExp constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("RegExp prototype is not an object")),
        }
    }

    fn install_regexp_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                REGEXP_DOT_ALL_PROPERTY,
                NativeFunctionKind::RegExpPrototypeDotAllGetter,
            ),
            (
                REGEXP_FLAGS_PROPERTY,
                NativeFunctionKind::RegExpPrototypeFlagsGetter,
            ),
            (
                REGEXP_GLOBAL_PROPERTY,
                NativeFunctionKind::RegExpPrototypeGlobalGetter,
            ),
            (
                REGEXP_HAS_INDICES_PROPERTY,
                NativeFunctionKind::RegExpPrototypeHasIndicesGetter,
            ),
            (
                REGEXP_IGNORE_CASE_PROPERTY,
                NativeFunctionKind::RegExpPrototypeIgnoreCaseGetter,
            ),
            (
                REGEXP_MULTILINE_PROPERTY,
                NativeFunctionKind::RegExpPrototypeMultilineGetter,
            ),
            (
                REGEXP_SOURCE_PROPERTY,
                NativeFunctionKind::RegExpPrototypeSourceGetter,
            ),
            (
                REGEXP_STICKY_PROPERTY,
                NativeFunctionKind::RegExpPrototypeStickyGetter,
            ),
            (
                REGEXP_UNICODE_PROPERTY,
                NativeFunctionKind::RegExpPrototypeUnicodeGetter,
            ),
            (
                REGEXP_UNICODE_SETS_PROPERTY,
                NativeFunctionKind::RegExpPrototypeUnicodeSetsGetter,
            ),
        ] {
            self.define_regexp_prototype_accessor(prototype, name, kind)?;
        }
        for (name, kind) in [
            (
                REGEXP_COMPILE_PROPERTY,
                NativeFunctionKind::RegExpPrototypeCompile,
            ),
            (
                REGEXP_PROTOTYPE_EXEC_NAME,
                NativeFunctionKind::RegExpPrototypeExec,
            ),
            (
                REGEXP_PROTOTYPE_TEST_NAME,
                NativeFunctionKind::RegExpPrototypeTest,
            ),
            (
                REGEXP_PROTOTYPE_TO_STRING_NAME,
                NativeFunctionKind::RegExpPrototypeToString,
            ),
        ] {
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
    }

    fn install_regexp_prototype_symbol_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (property, display, kind) in [
            (
                SYMBOL_MATCH_ALL_PROPERTY,
                "[Symbol.matchAll]",
                NativeFunctionKind::RegExpPrototypeSymbolMatchAll,
            ),
            (
                SYMBOL_MATCH_PROPERTY,
                "[Symbol.match]",
                NativeFunctionKind::RegExpPrototypeSymbolMatch,
            ),
            (
                SYMBOL_REPLACE_PROPERTY,
                "[Symbol.replace]",
                NativeFunctionKind::RegExpPrototypeSymbolReplace,
            ),
            (
                SYMBOL_SEARCH_PROPERTY,
                "[Symbol.search]",
                NativeFunctionKind::RegExpPrototypeSymbolSearch,
            ),
            (
                SYMBOL_SPLIT_PROPERTY,
                "[Symbol.split]",
                NativeFunctionKind::RegExpPrototypeSymbolSplit,
            ),
        ] {
            let key = self.regexp_well_known_symbol_property_key(property)?;
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.objects.define_property(
                prototype,
                key,
                display,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(method),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        Ok(())
    }

    fn define_regexp_prototype_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let getter = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            prototype,
            key,
            name,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_regexp_prototype_getter(
        &mut self,
        kind: NativeFunctionKind,
        this_value: &Value,
    ) -> Result<Value> {
        if kind == NativeFunctionKind::RegExpPrototypeFlagsGetter {
            return self.eval_regexp_prototype_flags_getter(this_value);
        }
        let regexp = self.regexp_receiver_data(this_value)?;
        let flags = parse_regexp_flags(regexp.flags())?;
        match kind {
            NativeFunctionKind::RegExpPrototypeDotAllGetter => Ok(Value::Bool(flags.dot_all())),
            NativeFunctionKind::RegExpPrototypeGlobalGetter => Ok(Value::Bool(flags.global())),
            NativeFunctionKind::RegExpPrototypeHasIndicesGetter => {
                Ok(Value::Bool(flags.has_indices()))
            }
            NativeFunctionKind::RegExpPrototypeIgnoreCaseGetter => {
                Ok(Value::Bool(flags.ignore_case()))
            }
            NativeFunctionKind::RegExpPrototypeMultilineGetter => {
                Ok(Value::Bool(flags.multiline()))
            }
            NativeFunctionKind::RegExpPrototypeSourceGetter => {
                let source = escaped_regexp_source_utf16(regexp.pattern_utf16());
                self.heap_utf16_string_value(&source)
            }
            NativeFunctionKind::RegExpPrototypeStickyGetter => Ok(Value::Bool(flags.sticky())),
            NativeFunctionKind::RegExpPrototypeUnicodeGetter => Ok(Value::Bool(flags.unicode())),
            NativeFunctionKind::RegExpPrototypeUnicodeSetsGetter => {
                Ok(Value::Bool(flags.unicode_sets()))
            }
            _ => Err(Error::runtime("native function is not a RegExp getter")),
        }
    }

    fn eval_regexp_prototype_flags_getter(&mut self, receiver: &Value) -> Result<Value> {
        if self.semantic_object_ref(receiver)?.is_none() {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        }
        let mut flags = String::new();
        for (property, marker) in [
            (REGEXP_HAS_INDICES_PROPERTY, 'd'),
            (REGEXP_GLOBAL_PROPERTY, 'g'),
            (REGEXP_IGNORE_CASE_PROPERTY, 'i'),
            (REGEXP_MULTILINE_PROPERTY, 'm'),
            (REGEXP_DOT_ALL_PROPERTY, 's'),
            (REGEXP_UNICODE_PROPERTY, 'u'),
            (REGEXP_UNICODE_SETS_PROPERTY, 'v'),
            (REGEXP_STICKY_PROPERTY, 'y'),
        ] {
            if to_boolean(&self.get_named(receiver, property)?) {
                flags.push(marker);
            }
        }
        self.heap_string_value(&flags)
    }

    fn regexp_receiver_data(&self, this_value: &Value) -> Result<RegExpValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        };
        self.objects
            .regexp_value(*id)?
            .cloned()
            .ok_or_else(|| Error::type_error(REGEXP_RECEIVER_ERROR))
    }

    fn regexp_test_code_units(&mut self, this_value: &Value, input: &[u16]) -> Result<bool> {
        let regexp = self.regexp_receiver_data(this_value)?;
        let flags = parse_regexp_flags(regexp.flags())?;
        self.charge_regexp_utf16_work(regexp.pattern_utf16(), input)?;
        let start = if flags.global() || flags.sticky() {
            self.regexp_last_index_utf16(this_value, input)?
        } else {
            0
        };
        let matched = regexp_test_utf16(regexp.pattern_utf16(), flags, input, start)?;
        let Some(range) = matched else {
            if flags.global() || flags.sticky() {
                self.set_regexp_last_index(this_value, 0)?;
            }
            return Ok(false);
        };
        if flags.global() || flags.sticky() {
            self.set_regexp_last_index(this_value, range.end)?;
        }
        Ok(true)
    }

    pub(in crate::runtime::native) fn regexp_exec(
        &mut self,
        this_value: &Value,
        input: &str,
    ) -> Result<Value> {
        let input = input.encode_utf16().collect::<Vec<_>>();
        self.regexp_exec_code_units(this_value, &input)
    }

    fn regexp_exec_code_units(&mut self, this_value: &Value, input: &[u16]) -> Result<Value> {
        let regexp = self.regexp_receiver_data(this_value)?;
        let flags = parse_regexp_flags(regexp.flags())?;
        self.charge_regexp_utf16_work(regexp.pattern_utf16(), input)?;
        let start = if flags.global() || flags.sticky() {
            self.regexp_last_index_utf16(this_value, input)?
        } else {
            0
        };
        let matched = regexp_find_utf16(regexp.pattern_utf16(), flags, input, start)?;
        let Some(matched) = matched else {
            if flags.global() || flags.sticky() {
                self.set_regexp_last_index(this_value, 0)?;
            }
            return Ok(Value::Null);
        };
        if flags.global() || flags.sticky() {
            self.set_regexp_last_index(this_value, matched.span.code_units.end)?;
        }
        self.regexp_match_array(input, &matched, flags.has_indices())
    }

    const fn discard_regexp_extra_args(_args: &[Value]) {}

    fn charge_regexp_utf16_work(&mut self, pattern: &[u16], input: &[u16]) -> Result<()> {
        let steps = pattern
            .len()
            .checked_add(input.len())
            .and_then(|steps| steps.checked_add(1))
            .ok_or_else(|| Error::limit("RegExp work estimate overflowed"))?;
        self.charge_runtime_steps(steps)
    }

    fn regexp_last_index(&mut self, this_value: &Value, input: &str) -> Result<usize> {
        let value = self.get_named(this_value, REGEXP_LAST_INDEX_PROPERTY)?;
        let index = self.to_length(&value)?;
        let input_length = u64::try_from(input.encode_utf16().count())
            .map_err(|_| Error::limit("RegExp input length exceeded supported range"))?;
        if index > input_length {
            let input_length = usize::try_from(input_length)
                .map_err(|_| Error::limit("RegExp input length exceeded supported range"))?;
            return Ok(input_length.saturating_add(1));
        }
        Self::length_to_usize(index, "RegExp lastIndex exceeded supported range")
    }

    fn regexp_last_index_utf16(&mut self, this_value: &Value, input: &[u16]) -> Result<usize> {
        let value = self.get_named(this_value, REGEXP_LAST_INDEX_PROPERTY)?;
        let index = self.to_length(&value)?;
        let input_length = u64::try_from(input.len())
            .map_err(|_| Error::limit("RegExp input length exceeded supported range"))?;
        if index > input_length {
            let input_length = usize::try_from(input_length)
                .map_err(|_| Error::limit("RegExp input length exceeded supported range"))?;
            return Ok(input_length.saturating_add(1));
        }
        Self::length_to_usize(index, "RegExp lastIndex exceeded supported range")
    }

    fn set_regexp_last_index(&mut self, this_value: &Value, index: usize) -> Result<()> {
        self.set_regexp_last_index_value(
            this_value,
            Value::Number(regexp_index_usize_to_number(index)?),
        )
    }

    fn set_regexp_last_index_value(&mut self, this_value: &Value, value: Value) -> Result<()> {
        let lookup = self.property_lookup(REGEXP_LAST_INDEX_PROPERTY);
        self.set(
            this_value,
            lookup,
            value,
            this_value,
            crate::runtime::abstract_operations::SetFailureBehavior::Throw,
        )
        .map(|_| ())
    }

    fn regexp_match_text(&mut self, pattern: &Value, input: &str) -> Result<Option<String>> {
        let result = self.regexp_exec(pattern, input)?;
        let Value::Object(id) = result else {
            return Ok(None);
        };
        let value = self.get_named(&Value::Object(id), "0")?;
        self.to_string(&value).map(Some)
    }

    fn regexp_match_all_results(&mut self, pattern: &Value, input: &str) -> Result<Vec<Value>> {
        let data = self.regexp_receiver_data(pattern)?;
        let flags = parse_regexp_flags(data.flags())?;
        let matcher = self.create_regexp_object_from_utf16(data.pattern_utf16(), data.flags())?;
        let start = self.regexp_last_index(pattern, input)?;
        self.set_regexp_last_index(&matcher, start)?;
        let mut results = Vec::new();
        if !flags.global() {
            let result = self.regexp_exec(&matcher, input)?;
            if !matches!(result, Value::Null) {
                results.push(result);
            }
            return Ok(results);
        }
        loop {
            let result = self.regexp_exec(&matcher, input)?;
            let Value::Object(id) = result else {
                return Ok(results);
            };
            let match_value = self.get_named(&Value::Object(id), "0")?;
            let match_text = self.to_string(&match_value)?;
            let is_empty = match_text.is_empty();
            results.push(Value::Object(id));
            if results.len() > self.limits.max_object_properties {
                return Err(Error::limit("RegExp matchAll result exceeded array limit"));
            }
            if is_empty {
                let index = self.regexp_last_index(&matcher, input)?;
                let next =
                    advance_utf16_index(input, index, flags.unicode() || flags.unicode_sets())?;
                self.set_regexp_last_index(&matcher, next)?;
            }
        }
    }

    fn regexp_string_array(&mut self, values: Vec<String>) -> Result<Value> {
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let mut elements = Vec::with_capacity(values.len());
        for value in values {
            elements.push(self.heap_string_value(&value)?);
        }
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn regexp_well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime::native) enum RegExpCallMode {
    Call,
    Construct,
}

const fn value_is_undefined(value: &Value) -> bool {
    matches!(value, Value::Undefined)
}

fn advance_utf16_index(text: &str, index: usize, unicode: bool) -> Result<usize> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let Some(first) = units.get(index).copied() else {
        return index
            .checked_add(1)
            .ok_or_else(|| Error::limit("RegExp string index overflowed"));
    };
    let width = if unicode
        && (0xD800..=0xDBFF).contains(&first)
        && units
            .get(index.saturating_add(1))
            .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
    {
        2
    } else {
        1
    };
    index
        .checked_add(width)
        .ok_or_else(|| Error::limit("RegExp string index overflowed"))
}
