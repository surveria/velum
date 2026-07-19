#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::ops::Range;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::same_value,
        call::RuntimeCallArgs,
        native::LegacyRegExpStaticKind,
        object::{AccessorPropertyUpdate, PropertyConfigurable, PropertyEnumerable},
    },
    value::Value,
};

use super::{NativeFunctionKind, engine::RegExpMatch};

impl Context {
    pub(super) fn install_legacy_regexp_static_accessors(
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
        if kind == LegacyRegExpStaticKind::Input {
            if let Some(input) = self.realm.regexp_statics.input() {
                return Ok(input.clone());
            }
            return self.heap_string_value("");
        }
        let units = self.legacy_regexp_static_units(kind)?.unwrap_or_default();
        self.heap_utf16_string_value(&units)
    }

    pub(in crate::runtime::native) fn eval_legacy_regexp_input_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.require_legacy_regexp_constructor(this_value)?;
        let value = args.as_slice().first().unwrap_or(&Value::Undefined);
        let input = self.to_utf16_string(value)?;
        let input = self.heap_utf16_string_value(&input)?;
        self.realm.regexp_statics.replace_input(input)?;
        Ok(Value::Undefined)
    }

    pub(super) fn record_legacy_regexp_match(
        &mut self,
        input: &[u16],
        matched: &RegExpMatch,
    ) -> Result<()> {
        let subject = self.heap_utf16_string_value(input)?;
        let mut captures = Vec::new();
        captures
            .try_reserve(matched.captures.len())
            .map_err(|error| {
                Error::limit(format!("legacy RegExp capture storage exhausted: {error}"))
            })?;
        captures.extend(
            matched
                .captures
                .iter()
                .map(|capture| capture.as_ref().map(|span| span.code_units.clone())),
        );
        self.realm
            .regexp_statics
            .replace_match(subject, matched.span.code_units.clone(), captures)
    }

    fn legacy_regexp_static_units(&self, kind: LegacyRegExpStaticKind) -> Result<Option<Vec<u16>>> {
        let state = &self.realm.regexp_statics;
        let Some(Value::String(subject)) = state.match_subject() else {
            return Ok(None);
        };
        let span = match kind {
            LegacyRegExpStaticKind::Input => return Ok(None),
            LegacyRegExpStaticKind::LastMatch => state.match_span().cloned(),
            LegacyRegExpStaticKind::LastParen => {
                state.captures().iter().rev().find_map(Clone::clone)
            }
            LegacyRegExpStaticKind::LeftContext => state.match_span().map(|matched| Range {
                start: 0,
                end: matched.start,
            }),
            LegacyRegExpStaticKind::RightContext => state.match_span().map(|matched| Range {
                start: matched.end,
                end: subject.as_utf16().len(),
            }),
            LegacyRegExpStaticKind::Capture(capture) => capture
                .checked_sub(1)
                .map(usize::from)
                .and_then(|index| state.captures().get(index))
                .and_then(Clone::clone),
        };
        let Some(span) = span else {
            return Ok(Some(Vec::new()));
        };
        subject
            .as_utf16()
            .get(span)
            .map(<[u16]>::to_vec)
            .map(Some)
            .ok_or_else(|| Error::runtime("legacy RegExp match span is outside its subject"))
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
}
