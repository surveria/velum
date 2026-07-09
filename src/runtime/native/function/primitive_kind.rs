use super::kind::NativeFunctionKind;

const BOOLEAN_PROTOTYPE_FUNCTION_LENGTH_ZERO: f64 = 0.0;
pub(in crate::runtime::native) const BOOLEAN_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime::native) const BOOLEAN_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
const NUMBER_PROTOTYPE_FUNCTION_LENGTH_ONE: f64 = 1.0;
const NUMBER_PROTOTYPE_FUNCTION_LENGTH_ZERO: f64 = 0.0;
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_TO_LOCALE_STRING_NAME: &str =
    "toLocaleString";
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_TO_FIXED_NAME: &str = "toFixed";
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_TO_EXPONENTIAL_NAME: &str = "toExponential";
pub(in crate::runtime::native) const NUMBER_PROTOTYPE_TO_PRECISION_NAME: &str = "toPrecision";
const SYMBOL_PROTOTYPE_FUNCTION_LENGTH_ZERO: f64 = 0.0;
const SYMBOL_PROTOTYPE_DESCRIPTION_GETTER_NAME: &str = "get description";
pub(in crate::runtime::native) const SYMBOL_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime::native) const SYMBOL_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn primitive_prototype_length(
        self,
    ) -> Option<f64> {
        match self {
            Self::NumberPrototypeToString
            | Self::NumberPrototypeToFixed
            | Self::NumberPrototypeToExponential
            | Self::NumberPrototypeToPrecision => Some(NUMBER_PROTOTYPE_FUNCTION_LENGTH_ONE),
            Self::BooleanPrototypeToString | Self::BooleanPrototypeValueOf => {
                Some(BOOLEAN_PROTOTYPE_FUNCTION_LENGTH_ZERO)
            }
            Self::NumberPrototypeToLocaleString | Self::NumberPrototypeValueOf => {
                Some(NUMBER_PROTOTYPE_FUNCTION_LENGTH_ZERO)
            }
            Self::SymbolPrototypeDescriptionGetter
            | Self::SymbolPrototypeToString
            | Self::SymbolPrototypeValueOf => Some(SYMBOL_PROTOTYPE_FUNCTION_LENGTH_ZERO),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn primitive_prototype_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::BooleanPrototypeToString => Some(BOOLEAN_PROTOTYPE_TO_STRING_NAME),
            Self::BooleanPrototypeValueOf => Some(BOOLEAN_PROTOTYPE_VALUE_OF_NAME),
            Self::NumberPrototypeToLocaleString => Some(NUMBER_PROTOTYPE_TO_LOCALE_STRING_NAME),
            Self::NumberPrototypeToString => Some(NUMBER_PROTOTYPE_TO_STRING_NAME),
            Self::NumberPrototypeValueOf => Some(NUMBER_PROTOTYPE_VALUE_OF_NAME),
            Self::NumberPrototypeToFixed => Some(NUMBER_PROTOTYPE_TO_FIXED_NAME),
            Self::NumberPrototypeToExponential => Some(NUMBER_PROTOTYPE_TO_EXPONENTIAL_NAME),
            Self::NumberPrototypeToPrecision => Some(NUMBER_PROTOTYPE_TO_PRECISION_NAME),
            Self::SymbolPrototypeDescriptionGetter => {
                Some(SYMBOL_PROTOTYPE_DESCRIPTION_GETTER_NAME)
            }
            Self::SymbolPrototypeToString => Some(SYMBOL_PROTOTYPE_TO_STRING_NAME),
            Self::SymbolPrototypeValueOf => Some(SYMBOL_PROTOTYPE_VALUE_OF_NAME),
            _ => None,
        }
    }
}
