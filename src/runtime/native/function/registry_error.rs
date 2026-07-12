use crate::value::ErrorName;

pub(super) const fn error_constructor_slot_index(name: ErrorName) -> usize {
    match name {
        ErrorName::Base => 58,
        ErrorName::EvalError => 59,
        ErrorName::RangeError => 60,
        ErrorName::ReferenceError => 61,
        ErrorName::SyntaxError => 62,
        ErrorName::Test262Error => 63,
        ErrorName::TypeError => 64,
        ErrorName::UriError => 65,
        ErrorName::AggregateError => 112,
        ErrorName::SuppressedError => 289,
    }
}
