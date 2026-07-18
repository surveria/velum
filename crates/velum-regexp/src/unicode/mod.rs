mod generated_core;

use core::cmp::Ordering;

/// Returns the pinned Unicode version used by the compiled tables.
#[must_use]
pub const fn unicode_version() -> &'static str {
    generated_core::UNICODE_VERSION
}

/// Returns whether a scalar value has the Unicode `ID_Start` property.
#[must_use]
pub fn is_id_start(value: u32) -> bool {
    contains(generated_core::ID_START, value)
}

/// Returns whether a scalar value has the Unicode `ID_Continue` property.
#[must_use]
pub fn is_id_continue(value: u32) -> bool {
    contains(generated_core::ID_CONTINUE, value)
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
