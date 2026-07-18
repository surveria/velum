mod generated_binary;
mod generated_case;
mod generated_core;
mod generated_general_category;
mod generated_script;
mod generated_script_extensions;

use core::cmp::Ordering;

/// Returns the pinned Unicode version used by the compiled tables.
#[must_use]
pub const fn unicode_version() -> &'static str {
    generated_core::UNICODE_VERSION
}

/// Returns whether a scalar value has the Unicode `ID_Start` property.
#[must_use]
pub fn is_id_start(value: u32) -> bool {
    binary_property_contains("ID_Start", value)
}

/// Returns whether a scalar value has the Unicode `ID_Continue` property.
#[must_use]
pub fn is_id_continue(value: u32) -> bool {
    binary_property_contains("ID_Continue", value)
}

/// Returns the ranges for an exact ECMAScript binary Unicode property name or
/// property alias.
///
/// Name matching is intentionally case-sensitive and does not apply Unicode
/// loose matching because ECMAScript does not permit it in property escapes.
#[must_use]
pub fn binary_property_ranges(name: &str) -> Option<&'static [(u32, u32)]> {
    generated_binary::binary_property_ranges(name)
}

/// Returns whether a code point belongs to an exact ECMAScript binary Unicode
/// property name or property alias.
#[must_use]
pub fn binary_property_contains(name: &str, value: u32) -> bool {
    binary_property_ranges(name).is_some_and(|ranges| contains(ranges, value))
}

/// Resolves an exact ECMAScript Unicode property expression.
///
/// A missing property name accepts a binary property or a General Category
/// value. Named expressions accept only the property names and aliases listed
/// by ECMAScript.
#[must_use]
pub fn unicode_property_ranges(
    property_name: Option<&str>,
    property_value: &str,
) -> Option<&'static [(u32, u32)]> {
    match property_name {
        None => binary_property_ranges(property_value)
            .or_else(|| generated_general_category::general_category_ranges(property_value)),
        Some("General_Category" | "gc") => {
            generated_general_category::general_category_ranges(property_value)
        }
        Some("Script" | "sc") => generated_script::script_ranges(property_value),
        Some("Script_Extensions" | "scx") => {
            generated_script_extensions::script_extension_ranges(property_value)
        }
        Some(_) => None,
    }
}

#[must_use]
pub fn canonicalize(value: u32, unicode: bool) -> u32 {
    if unicode {
        generated_case::simple_case_fold(value)
    } else {
        let uppercase = generated_case::legacy_uppercase(value);
        if value >= 0x80 && uppercase < 0x80 {
            value
        } else {
            uppercase
        }
    }
}

#[must_use]
pub fn case_closure_contains(ranges: &[(u32, u32)], value: u32, unicode: bool) -> bool {
    let canonical = canonicalize(value, unicode);
    contains(ranges, canonical)
        || if unicode {
            generated_case::simple_fold_source_in_ranges(canonical, ranges)
        } else {
            generated_case::legacy_uppercase_source_in_ranges(canonical, ranges)
        }
}

#[must_use]
pub fn case_closure_all_in_ranges(ranges: &[(u32, u32)], value: u32) -> bool {
    let canonical = canonicalize(value, true);
    contains(ranges, canonical)
        && generated_case::simple_fold_sources_all_in_ranges(canonical, ranges)
}

fn contains(ranges: &[(u32, u32)], value: u32) -> bool {
    ranges
        .binary_search_by(|(start, end)| {
            if value < *start {
                Ordering::Greater
            } else if value > *end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}
