use crate::{
    error::Result,
    runtime::{Context, object::OwnPropertyDescriptor},
    value::Value,
};

use super::{
    NativeFunctionKind, REGEXP_DOT_ALL_PROPERTY, REGEXP_GLOBAL_PROPERTY,
    REGEXP_HAS_INDICES_PROPERTY, REGEXP_IGNORE_CASE_PROPERTY, REGEXP_MULTILINE_PROPERTY,
    REGEXP_STICKY_PROPERTY, REGEXP_UNICODE_PROPERTY, REGEXP_UNICODE_SETS_PROPERTY,
    parse_regexp_flags,
};

impl Context {
    pub(super) fn intrinsic_regexp_flags(&mut self, receiver: &Value) -> Result<Option<String>> {
        let Value::Object(receiver_id) = receiver else {
            return Ok(None);
        };
        let Some(regexp) = self.objects.regexp_value(*receiver_id)?.cloned() else {
            return Ok(None);
        };
        let prototype_id = self.regexp_constructor_prototype()?;
        if self.objects.prototype_value(*receiver_id)? != Value::Object(prototype_id) {
            return Ok(None);
        }

        for (property, expected_kind) in intrinsic_flag_getters() {
            let lookup = self.property_lookup(property);
            if self.objects.has_own(*receiver_id, lookup)? {
                return Ok(None);
            }
            let Some(OwnPropertyDescriptor::Accessor(descriptor)) =
                self.objects.own_property_descriptor(prototype_id, lookup)?
            else {
                return Ok(None);
            };
            let Value::NativeFunction(getter_id) = descriptor.get_ref() else {
                return Ok(None);
            };
            if self.native_function(*getter_id)?.kind() != expected_kind {
                return Ok(None);
            }
        }

        let flags = parse_regexp_flags(regexp.flags())?;
        let mut text = String::new();
        for (enabled, marker) in [
            (flags.has_indices(), 'd'),
            (flags.global(), 'g'),
            (flags.ignore_case(), 'i'),
            (flags.multiline(), 'm'),
            (flags.dot_all(), 's'),
            (flags.unicode(), 'u'),
            (flags.unicode_sets(), 'v'),
            (flags.sticky(), 'y'),
        ] {
            if enabled {
                text.push(marker);
            }
        }
        Ok(Some(text))
    }
}

const fn intrinsic_flag_getters() -> [(&'static str, NativeFunctionKind); 8] {
    [
        (
            REGEXP_HAS_INDICES_PROPERTY,
            NativeFunctionKind::RegExpPrototypeHasIndicesGetter,
        ),
        (
            REGEXP_GLOBAL_PROPERTY,
            NativeFunctionKind::RegExpPrototypeGlobalGetter,
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
            REGEXP_DOT_ALL_PROPERTY,
            NativeFunctionKind::RegExpPrototypeDotAllGetter,
        ),
        (
            REGEXP_UNICODE_PROPERTY,
            NativeFunctionKind::RegExpPrototypeUnicodeGetter,
        ),
        (
            REGEXP_UNICODE_SETS_PROPERTY,
            NativeFunctionKind::RegExpPrototypeUnicodeSetsGetter,
        ),
        (
            REGEXP_STICKY_PROPERTY,
            NativeFunctionKind::RegExpPrototypeStickyGetter,
        ),
    ]
}
