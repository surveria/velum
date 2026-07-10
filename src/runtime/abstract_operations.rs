mod conversion;
mod equality;

pub(in crate::runtime) use conversion::{PreferredType, is_primitive};
pub(in crate::runtime) use equality::{
    abstract_equality, number_same_value_zero, number_strict_equality, same_value, same_value_zero,
    strict_equality,
};
