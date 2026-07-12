use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
    },
    value::{ObjectId, Value},
};

use super::{
    NativeFunctionKind, STRING_PROTOTYPE_IS_WELL_FORMED_NAME, STRING_PROTOTYPE_ITERATOR_NAME,
    STRING_PROTOTYPE_MATCH_ALL_NAME, STRING_PROTOTYPE_REPLACE_ALL_NAME,
    STRING_PROTOTYPE_TO_WELL_FORMED_NAME,
};

const REPLACEMENT_MARKER: u16 = 0x24;
const REPLACEMENT_CHARACTER: u16 = 0xFFFD;
const STRING_METHOD_NULLISH_RECEIVER_ERROR: &str =
    "String.prototype method cannot convert undefined or null to object";
const SYMBOL_MATCH_PROPERTY: &str = "match";
const SYMBOL_MATCH_ALL_PROPERTY: &str = "matchAll";
const SYMBOL_REPLACE_PROPERTY: &str = "replace";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const STRING_ITERATOR_TAG: &str = "String Iterator";
const STRING_ITERATOR_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const STRING_ITERATOR_RECEIVER_ERROR: &str =
    "String Iterator.prototype.next requires a String Iterator receiver";
const STRING_ITERATOR_STATE_PROPERTY: &str = "\0StringIteratorState";

impl Context {
    pub(in crate::runtime::native) fn eval_modern_string_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::StringPrototypeIsWellFormed => {
                Some(self.eval_string_prototype_is_well_formed(args, this_value))
            }
            NativeFunctionKind::StringPrototypeIterator => {
                Some(self.eval_string_prototype_iterator(args, this_value))
            }
            NativeFunctionKind::StringPrototypeMatchAll => {
                Some(self.eval_string_prototype_match_all(args, this_value))
            }
            NativeFunctionKind::StringPrototypeReplaceAll => {
                Some(self.eval_string_prototype_replace_all(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToWellFormed => {
                Some(self.eval_string_prototype_to_well_formed(args, this_value))
            }
            NativeFunctionKind::StringIteratorNext => {
                Some(self.eval_string_iterator_next(args, this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime::native) fn install_string_modern_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        for (name, kind) in [
            (
                STRING_PROTOTYPE_IS_WELL_FORMED_NAME,
                NativeFunctionKind::StringPrototypeIsWellFormed,
            ),
            (
                STRING_PROTOTYPE_MATCH_ALL_NAME,
                NativeFunctionKind::StringPrototypeMatchAll,
            ),
            (
                STRING_PROTOTYPE_REPLACE_ALL_NAME,
                NativeFunctionKind::StringPrototypeReplaceAll,
            ),
            (
                STRING_PROTOTYPE_TO_WELL_FORMED_NAME,
                NativeFunctionKind::StringPrototypeToWellFormed,
            ),
        ] {
            let function = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, function)?;
        }
        self.install_string_iterator_method(prototype)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_is_well_formed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_modern_string_args(args.as_slice());
        let units = self.string_receiver_utf16(this_value)?;
        Ok(Value::Bool(string_is_well_formed(&units)))
    }

    pub(in crate::runtime::native) fn eval_string_prototype_to_well_formed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_modern_string_args(args.as_slice());
        let units = self.string_receiver_utf16(this_value)?;
        let output = string_to_well_formed(&units);
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_replace_all(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(STRING_METHOD_NULLISH_RECEIVER_ERROR));
        }
        let search_value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let replace_value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !matches!(search_value, Value::Undefined | Value::Null) {
            self.require_global_regexp_for_replace_all(&search_value)?;
            if self.semantic_object_ref(&search_value)?.is_some()
                && let Some(replacer) =
                    self.string_well_known_method(&search_value, SYMBOL_REPLACE_PROPERTY)?
            {
                return self.call_value(
                    &replacer,
                    &[this_value.clone(), replace_value],
                    search_value,
                );
            }
        }

        let string = self.string_receiver_utf16(this_value)?;
        let search = self.string_argument_utf16(&search_value)?;
        let functional_replace = self.semantic_is_callable(&replace_value)?;
        let replacement = if functional_replace {
            None
        } else {
            Some(self.string_argument_utf16(&replace_value)?)
        };
        let positions = replace_all_positions(&string, &search);
        let matched = self.heap_utf16_string_value(&search)?;
        let whole = self.heap_utf16_string_value(&string)?;
        let mut output = Vec::new();
        let mut cursor = 0usize;
        for position in positions {
            append_utf16_slice(&mut output, &string, cursor, position)?;
            let substitution = if functional_replace {
                let position = Value::Number(Self::usize_to_number(
                    position,
                    "String replacement position exceeded numeric range",
                )?);
                let result = self.call_value(
                    &replace_value,
                    &[matched.clone(), position, whole.clone()],
                    Value::Undefined,
                )?;
                self.string_argument_utf16(&result)?
            } else {
                plain_replacement_substitution(
                    replacement.as_deref().unwrap_or_default(),
                    &search,
                    &string,
                    position,
                )?
            };
            output.extend_from_slice(&substitution);
            self.check_utf16_string_len(&output)?;
            cursor = position
                .checked_add(search.len())
                .ok_or_else(|| Error::limit("String replacement cursor overflowed"))?;
        }
        append_utf16_slice(&mut output, &string, cursor, string.len())?;
        self.check_utf16_string_len(&output)?;
        self.heap_utf16_string_value(&output)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_match_all(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(STRING_METHOD_NULLISH_RECEIVER_ERROR));
        }
        let regexp = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if self.semantic_object_ref(&regexp)?.is_some() {
            self.require_global_regexp_for_match_all(&regexp)?;
            if let Some(matcher) =
                self.string_well_known_method(&regexp, SYMBOL_MATCH_ALL_PROPERTY)?
            {
                return self.call_value(&matcher, std::slice::from_ref(this_value), regexp);
            }
        }

        let string = self.string_receiver_utf16(this_value)?;
        let string = self.heap_utf16_string_value(&string)?;
        let _string_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&string))?;
        let flags = self.heap_string_value("g")?;
        let constructor = self.regexp_constructor_value()?;
        let matcher =
            self.semantic_construct(&constructor, &[regexp, flags], constructor.clone())?;
        let _matcher_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&matcher))?;
        let method = self
            .string_well_known_method(&matcher, SYMBOL_MATCH_ALL_PROPERTY)?
            .ok_or_else(|| Error::type_error("RegExp Symbol.matchAll method is not callable"))?;
        self.call_value(&method, &[string], matcher)
    }

    pub(in crate::runtime::native) fn eval_string_prototype_iterator(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_modern_string_args(args.as_slice());
        let units = self.string_receiver_utf16(this_value)?;
        let mut items = Vec::new();
        let mut index = 0usize;
        while let Some(unit) = units.get(index).copied() {
            let width = if is_high_surrogate(unit)
                && units
                    .get(index.saturating_add(1))
                    .copied()
                    .is_some_and(is_low_surrogate)
            {
                2
            } else {
                1
            };
            let end = index
                .checked_add(width)
                .ok_or_else(|| Error::limit("String iterator index overflowed"))?;
            let item = units
                .get(index..end)
                .ok_or_else(|| Error::runtime("String iterator slice is out of bounds"))?;
            items.push(self.heap_utf16_string_value(item)?);
            index = end;
        }
        self.create_string_iterator_object(items)
    }

    pub(in crate::runtime::native) fn eval_string_iterator_next(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_modern_string_args(args.as_slice());
        if !matches!(this_value, Value::Object(_)) {
            return Err(Error::type_error(STRING_ITERATOR_RECEIVER_ERROR));
        }
        let key = self.intern_property_key(STRING_ITERATOR_STATE_PROPERTY)?;
        let property =
            DynamicPropertyKey::new(STRING_ITERATOR_STATE_PROPERTY.to_owned(), Some(key));
        let Some(OwnPropertyDescriptor::Data(descriptor)) =
            self.semantic_own_property_descriptor(this_value, &property)?
        else {
            return Err(Error::type_error(STRING_ITERATOR_RECEIVER_ERROR));
        };
        let next = descriptor.value();
        let Value::NativeFunction(id) = next else {
            return Err(Error::type_error(STRING_ITERATOR_RECEIVER_ERROR));
        };
        let NativeFunctionKind::CollectionIteratorNext(iterator) = self.native_function(id)?.kind()
        else {
            return Err(Error::type_error(STRING_ITERATOR_RECEIVER_ERROR));
        };
        self.eval_collection_iterator_next_state(iterator, iterator)
    }

    fn install_string_iterator_method(&mut self, prototype: ObjectId) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let function = self.create_ephemeral_native_function(
            NativeFunctionKind::StringPrototypeIterator,
            Value::Undefined,
        )?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            STRING_PROTOTYPE_ITERATOR_NAME,
            builtin_method_update(function),
            self.limits.max_object_properties,
        )
    }

    fn create_string_iterator_object(&mut self, items: Vec<Value>) -> Result<Value> {
        let iterator_parent = self.iterator_prototype_object_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype(
            Some(iterator_parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("String iterator prototype creation failed"));
        };
        let iterator = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(owner) = iterator else {
            return Err(Error::runtime("String iterator creation failed"));
        };
        let state = self.create_collection_iterator(items)?;
        let state_next = self.create_ephemeral_native_function(
            NativeFunctionKind::CollectionIteratorNext(state),
            Value::Undefined,
        )?;
        let state_key = self.intern_property_key(STRING_ITERATOR_STATE_PROPERTY)?;
        self.objects.define_property(
            owner,
            state_key,
            STRING_ITERATOR_STATE_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(state_next),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )?;
        let next = self.create_ephemeral_native_function(
            NativeFunctionKind::StringIteratorNext,
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype_id, "next", next)?;
        self.install_string_iterator_symbols(prototype_id)?;
        Ok(Value::Object(owner))
    }

    fn install_string_iterator_symbols(&mut self, prototype: ObjectId) -> Result<()> {
        let self_method = self
            .create_ephemeral_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
        let Some(iterator_symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(iterator_symbol),
            STRING_PROTOTYPE_ITERATOR_NAME,
            builtin_method_update(self_method),
            self.limits.max_object_properties,
        )?;
        let tag_key = self.string_well_known_symbol_key(SYMBOL_TO_STRING_TAG_PROPERTY)?;
        let tag = self.heap_string_value(STRING_ITERATOR_TAG)?;
        self.objects.define_property(
            prototype,
            tag_key,
            STRING_ITERATOR_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn require_global_regexp_for_replace_all(&mut self, search: &Value) -> Result<()> {
        if !self.string_is_regexp(search)? {
            return Ok(());
        }
        let flags = self.get_named(search, "flags")?;
        if matches!(flags, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "String.prototype.replaceAll RegExp flags are nullish",
            ));
        }
        let flags = self.to_string(&flags)?;
        if !flags.contains('g') {
            return Err(Error::type_error(
                "String.prototype.replaceAll requires a global RegExp",
            ));
        }
        Ok(())
    }

    fn require_global_regexp_for_match_all(&mut self, regexp: &Value) -> Result<()> {
        if !self.string_is_regexp(regexp)? {
            return Ok(());
        }
        let flags = self.get_named(regexp, "flags")?;
        if matches!(flags, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "String.prototype.matchAll RegExp flags are nullish",
            ));
        }
        if !self.to_string(&flags)?.contains('g') {
            return Err(Error::type_error(
                "String.prototype.matchAll requires a global RegExp",
            ));
        }
        Ok(())
    }

    fn string_is_regexp(&mut self, value: &Value) -> Result<bool> {
        if self.semantic_object_ref(value)?.is_none() {
            return Ok(false);
        }
        let matcher = self.string_well_known_value(value, SYMBOL_MATCH_PROPERTY)?;
        if !matches!(matcher, Value::Undefined) {
            return Ok(to_boolean(&matcher));
        }
        let Value::Object(id) = value else {
            return Ok(false);
        };
        self.objects
            .regexp_value(*id)
            .map(|regexp| regexp.is_some())
    }

    fn string_well_known_method(&mut self, value: &Value, property: &str) -> Result<Option<Value>> {
        let key = self.string_well_known_symbol_key(property)?;
        self.get_method(
            value,
            PropertyLookup::from_key(symbol_display_name(property), key),
        )
    }

    fn string_well_known_value(&mut self, value: &Value, property: &str) -> Result<Value> {
        let key = self.string_well_known_symbol_key(property)?;
        self.get(
            value,
            PropertyLookup::from_key(symbol_display_name(property), key),
        )
    }

    fn string_well_known_symbol_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }

    const fn discard_modern_string_args(_args: &[Value]) {}
}

fn symbol_display_name(property: &str) -> &str {
    match property {
        SYMBOL_MATCH_PROPERTY => "Symbol(Symbol.match)",
        SYMBOL_MATCH_ALL_PROPERTY => "Symbol(Symbol.matchAll)",
        SYMBOL_REPLACE_PROPERTY => "Symbol(Symbol.replace)",
        _ => "Symbol",
    }
}

const fn builtin_method_update(value: Value) -> PropertyUpdate {
    PropertyUpdate::Data(DataPropertyUpdate::new(
        Some(value),
        Some(PropertyWritable::Yes),
        Some(PropertyEnumerable::No),
        Some(PropertyConfigurable::Yes),
    ))
}

fn replace_all_positions(string: &[u16], search: &[u16]) -> Vec<usize> {
    if search.is_empty() {
        return (0..=string.len()).collect();
    }
    let mut positions = Vec::new();
    let mut cursor = 0usize;
    while let Some(candidate) = string.get(cursor..) {
        let Some(offset) = candidate
            .windows(search.len())
            .position(|window| window == search)
        else {
            break;
        };
        let position = cursor.saturating_add(offset);
        positions.push(position);
        cursor = position.saturating_add(search.len());
    }
    positions
}

fn plain_replacement_substitution(
    replacement: &[u16],
    matched: &[u16],
    string: &[u16],
    position: usize,
) -> Result<Vec<u16>> {
    let tail = position
        .checked_add(matched.len())
        .ok_or_else(|| Error::limit("String replacement tail overflowed"))?;
    let mut output = Vec::new();
    let mut index = 0usize;
    while let Some(unit) = replacement.get(index).copied() {
        if unit != REPLACEMENT_MARKER {
            output.push(unit);
            index = index.saturating_add(1);
            continue;
        }
        let Some(next) = replacement.get(index.saturating_add(1)).copied() else {
            output.push(unit);
            break;
        };
        match next {
            0x24 => output.push(REPLACEMENT_MARKER),
            0x26 => output.extend_from_slice(matched),
            0x60 => output.extend_from_slice(
                string
                    .get(..position)
                    .ok_or_else(|| Error::runtime("String replacement prefix is out of bounds"))?,
            ),
            0x27 => output.extend_from_slice(
                string
                    .get(tail..)
                    .ok_or_else(|| Error::runtime("String replacement suffix is out of bounds"))?,
            ),
            _ => {
                output.push(unit);
                index = index.saturating_add(1);
                continue;
            }
        }
        index = index.saturating_add(2);
    }
    Ok(output)
}

fn append_utf16_slice(
    output: &mut Vec<u16>,
    source: &[u16],
    start: usize,
    end: usize,
) -> Result<()> {
    let slice = source
        .get(start..end)
        .ok_or_else(|| Error::runtime("String replacement slice is out of bounds"))?;
    output.extend_from_slice(slice);
    Ok(())
}

fn string_is_well_formed(units: &[u16]) -> bool {
    let mut index = 0usize;
    while let Some(unit) = units.get(index).copied() {
        if is_high_surrogate(unit) {
            if !units
                .get(index.saturating_add(1))
                .copied()
                .is_some_and(is_low_surrogate)
            {
                return false;
            }
            index = index.saturating_add(2);
            continue;
        }
        if is_low_surrogate(unit) {
            return false;
        }
        index = index.saturating_add(1);
    }
    true
}

fn string_to_well_formed(units: &[u16]) -> Vec<u16> {
    let mut output = Vec::with_capacity(units.len());
    let mut index = 0usize;
    while let Some(unit) = units.get(index).copied() {
        if is_high_surrogate(unit)
            && let Some(low) = units.get(index.saturating_add(1)).copied()
            && is_low_surrogate(low)
        {
            output.push(unit);
            output.push(low);
            index = index.saturating_add(2);
            continue;
        }
        output.push(if is_high_surrogate(unit) || is_low_surrogate(unit) {
            REPLACEMENT_CHARACTER
        } else {
            unit
        });
        index = index.saturating_add(1);
    }
    output
}

const fn is_high_surrogate(unit: u16) -> bool {
    unit >= 0xD800 && unit <= 0xDBFF
}

const fn is_low_surrogate(unit: u16) -> bool {
    unit >= 0xDC00 && unit <= 0xDFFF
}
