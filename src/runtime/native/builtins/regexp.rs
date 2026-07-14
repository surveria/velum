use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{same_value, to_boolean},
        call::RuntimeCallArgs,
        native::LegacyRegExpStaticKind,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
            RegExpValue,
        },
    },
    value::{ObjectId, Value},
};

mod compile;
mod engine;
mod escape;
mod flags;
mod match_iterator;
mod match_result;
mod match_search;
mod replace;
mod split;

use engine::{
    compile_regexp_pattern_utf16, escaped_regexp_source_utf16, parse_regexp_flags,
    regexp_find_utf16, regexp_index_usize_to_number, regexp_test_utf16,
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, REGEXP_NAME, REGEXP_PROTOTYPE_EXEC_NAME,
    REGEXP_PROTOTYPE_TEST_NAME, REGEXP_PROTOTYPE_TO_STRING_NAME,
};

const REGEXP_ESCAPE_NAME: &str = "escape";
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
const SYMBOL_MATCH_DISPLAY: &str = "[Symbol.match]";
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
        self.install_regexp_static_methods(id)?;
        self.install_legacy_regexp_static_accessors(id)?;
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
        let pattern_value = args.first().cloned().unwrap_or(Value::Undefined);
        let flags_value = args.get(1);
        let pattern_is_regexp = self.is_regexp(&pattern_value)?;
        if mode == RegExpCallMode::Call
            && pattern_is_regexp
            && flags_value.is_none_or(value_is_undefined)
        {
            let pattern_constructor =
                self.get_named(&pattern_value, OBJECT_CONSTRUCTOR_PROPERTY)?;
            let active_constructor = self.regexp_constructor_value()?;
            if same_value(&pattern_constructor, &active_constructor) {
                return Ok(pattern_value);
            }
        }
        let pattern = if pattern_is_regexp {
            let source = self.get_named(&pattern_value, REGEXP_SOURCE_PROPERTY)?;
            self.to_utf16_string(&source)?
        } else {
            match &pattern_value {
                Value::Undefined => Vec::new(),
                value => self.to_utf16_string(value)?,
            }
        };
        let flags = match (pattern_is_regexp, flags_value) {
            (true, None | Some(Value::Undefined)) => {
                let flags = self.get_named(&pattern_value, REGEXP_FLAGS_PROPERTY)?;
                self.to_string(&flags)?
            }
            (false, None | Some(Value::Undefined)) => String::new(),
            (_, Some(value)) => self.to_string(value)?,
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

    fn create_regexp_object_from_text(&mut self, pattern: &str, flags: &str) -> Result<Value> {
        let pattern = pattern.encode_utf16().collect::<Vec<_>>();
        self.create_regexp_object_from_utf16(&pattern, flags)
    }

    fn create_regexp_object_from_utf16(&mut self, pattern: &[u16], flags: &str) -> Result<Value> {
        self.charge_regexp_utf16_work(pattern, &[])?;
        let parsed_flags = parse_regexp_flags(flags)?;
        let compiled = compile_regexp_pattern_utf16(pattern, parsed_flags)?;
        self.check_utf16_string_len(pattern)?;
        self.check_string_len(flags)?;
        let prototype = self.regexp_constructor_prototype()?;
        let id = self.objects.create_regexp(
            RegExpValue::new_utf16(pattern.to_vec(), parsed_flags, compiled)?,
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

    fn install_legacy_regexp_static_accessors(
        &mut self,
        constructor: crate::value::NativeFunctionId,
    ) -> Result<()> {
        for (name, kind, writable) in [
            ("input", LegacyRegExpStaticKind::Input, true),
            ("$_", LegacyRegExpStaticKind::Input, true),
            ("lastMatch", LegacyRegExpStaticKind::LastMatch, false),
            ("$&", LegacyRegExpStaticKind::LastMatch, false),
            ("lastParen", LegacyRegExpStaticKind::LastParen, false),
            ("$+", LegacyRegExpStaticKind::LastParen, false),
            ("leftContext", LegacyRegExpStaticKind::LeftContext, false),
            ("$`", LegacyRegExpStaticKind::LeftContext, false),
            ("rightContext", LegacyRegExpStaticKind::RightContext, false),
            ("$'", LegacyRegExpStaticKind::RightContext, false),
        ] {
            self.define_legacy_regexp_static_accessor(constructor, name, kind, writable)?;
        }
        for capture in 1_u8..=9 {
            let name = format!("${capture}");
            self.define_legacy_regexp_static_accessor(
                constructor,
                &name,
                LegacyRegExpStaticKind::Capture(capture),
                false,
            )?;
        }
        Ok(())
    }

    fn define_legacy_regexp_static_accessor(
        &mut self,
        constructor: crate::value::NativeFunctionId,
        name: &str,
        kind: LegacyRegExpStaticKind,
        writable: bool,
    ) -> Result<()> {
        let getter = self.create_ephemeral_native_function(
            NativeFunctionKind::RegExpLegacyGetter(kind),
            Value::Undefined,
        )?;
        let setter = if writable {
            Some(self.create_ephemeral_native_function(
                NativeFunctionKind::RegExpLegacyInputSetter,
                Value::Undefined,
            )?)
        } else {
            None
        };
        let key = self.intern_property_key(name)?;
        self.define_native_function_accessor_property_key(
            constructor,
            name,
            key,
            AccessorPropertyUpdate::new(
                Some(getter),
                setter,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }

    pub(in crate::runtime::native) fn eval_legacy_regexp_static_getter(
        &mut self,
        kind: LegacyRegExpStaticKind,
        this_value: &Value,
    ) -> Result<Value> {
        self.require_legacy_regexp_constructor(this_value)?;
        match kind {
            LegacyRegExpStaticKind::Input
            | LegacyRegExpStaticKind::LastMatch
            | LegacyRegExpStaticKind::LastParen
            | LegacyRegExpStaticKind::LeftContext
            | LegacyRegExpStaticKind::RightContext
            | LegacyRegExpStaticKind::Capture(_) => self.heap_string_value(""),
        }
    }

    pub(in crate::runtime::native) fn eval_legacy_regexp_input_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.require_legacy_regexp_constructor(this_value)?;
        let value = args.as_slice().first().unwrap_or(&Value::Undefined);
        self.to_utf16_string(value)?;
        Ok(Value::Undefined)
    }

    fn require_legacy_regexp_constructor(&mut self, this_value: &Value) -> Result<()> {
        let constructor = self.regexp_constructor_value()?;
        if same_value(&constructor, this_value) {
            return Ok(());
        }
        Err(Error::type_error(
            "legacy RegExp accessor requires the intrinsic RegExp constructor",
        ))
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
        if self.is_regexp_prototype(this_value)? {
            return if kind == NativeFunctionKind::RegExpPrototypeSourceGetter {
                self.heap_string_value("(?:)")
            } else {
                Ok(Value::Undefined)
            };
        }
        let regexp = self.regexp_receiver_data(this_value)?;
        let flags = regexp.parsed_flags();
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

    fn is_regexp_prototype(&mut self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        self.regexp_constructor_prototype()
            .map(|prototype| *id == prototype)
    }

    fn eval_regexp_prototype_flags_getter(&mut self, receiver: &Value) -> Result<Value> {
        if self.semantic_object_ref(receiver)?.is_none() {
            return Err(Error::type_error(REGEXP_RECEIVER_ERROR));
        }
        if let Some(flags) = self.intrinsic_regexp_flags(receiver)? {
            return self.heap_string_value(&flags);
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
            let enabled = self.get_named(receiver, property)?;
            if to_boolean(self, &enabled)? {
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
        let flags = regexp.parsed_flags();
        self.charge_regexp_utf16_work(regexp.pattern_utf16(), input)?;
        let last_index = self.regexp_last_index_utf16(this_value, input)?;
        let start = if flags.global() || flags.sticky() {
            last_index
        } else {
            0
        };
        let matched = regexp_test_utf16(regexp.compiled(), flags, input, start);
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

    fn regexp_exec_code_units(&mut self, this_value: &Value, input: &[u16]) -> Result<Value> {
        let regexp = self.regexp_receiver_data(this_value)?;
        let flags = regexp.parsed_flags();
        self.charge_regexp_utf16_work(regexp.pattern_utf16(), input)?;
        let last_index = self.regexp_last_index_utf16(this_value, input)?;
        let start = if flags.global() || flags.sticky() {
            last_index
        } else {
            0
        };
        let matched = regexp_find_utf16(regexp.compiled(), flags, input, start);
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

    fn regexp_well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }

    fn is_regexp(&mut self, value: &Value) -> Result<bool> {
        if self.semantic_object_ref(value)?.is_none() {
            return Ok(false);
        }
        let key = self.regexp_well_known_symbol_property_key(SYMBOL_MATCH_PROPERTY)?;
        let matcher = self.get(value, PropertyLookup::from_key(SYMBOL_MATCH_DISPLAY, key))?;
        if !matches!(matcher, Value::Undefined) {
            return to_boolean(self, &matcher);
        }
        let Value::Object(id) = value else {
            return Ok(false);
        };
        self.objects
            .regexp_value(*id)
            .map(|regexp| regexp.is_some())
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
