use super::{
    ARRAY_BUFFER_FUNCTION_LENGTH, ARRAY_BUFFER_NAME, ASYNC_FUNCTION_FUNCTION_LENGTH,
    ASYNC_FUNCTION_NAME, ASYNC_GENERATOR_FUNCTION_FUNCTION_LENGTH, ASYNC_GENERATOR_FUNCTION_NAME,
    BIGINT_AS_INT_N_NAME, BIGINT_AS_UINT_N_NAME, BIGINT_FUNCTION_LENGTH, BIGINT_NAME,
    BOOLEAN_FUNCTION_LENGTH, BOOLEAN_NAME, BOUND_FUNCTION_LENGTH, BOUND_FUNCTION_NAME,
    ERROR_PROTOTYPE_TO_STRING_LENGTH, ERROR_PROTOTYPE_TO_STRING_NAME, EVAL_FUNCTION_LENGTH,
    EVAL_NAME, FUNCTION_FUNCTION_LENGTH, FUNCTION_NAME, FUNCTION_PROTOTYPE_APPLY_LENGTH,
    FUNCTION_PROTOTYPE_APPLY_NAME, FUNCTION_PROTOTYPE_BIND_LENGTH, FUNCTION_PROTOTYPE_BIND_NAME,
    FUNCTION_PROTOTYPE_CALL_LENGTH, FUNCTION_PROTOTYPE_CALL_NAME,
    FUNCTION_PROTOTYPE_HAS_INSTANCE_LENGTH, FUNCTION_PROTOTYPE_HAS_INSTANCE_NAME,
    FUNCTION_PROTOTYPE_TO_STRING_LENGTH, FUNCTION_PROTOTYPE_TO_STRING_NAME,
    JSON_IS_RAW_JSON_FUNCTION_LENGTH, JSON_IS_RAW_JSON_NAME, JSON_PARSE_FUNCTION_LENGTH,
    JSON_PARSE_NAME, JSON_RAW_JSON_FUNCTION_LENGTH, JSON_RAW_JSON_NAME,
    JSON_STRINGIFY_FUNCTION_LENGTH, JSON_STRINGIFY_NAME, NUMBER_FUNCTION_LENGTH, NUMBER_NAME,
    NativeFunctionKind, PROXY_FUNCTION_LENGTH, PROXY_NAME, PROXY_REVOCABLE_FUNCTION_LENGTH,
    PROXY_REVOCABLE_NAME, PROXY_REVOKE_FUNCTION_LENGTH, PROXY_REVOKE_NAME,
    SPECIES_GETTER_FUNCTION_LENGTH, SPECIES_GETTER_NAME, STRING_FUNCTION_LENGTH, STRING_NAME,
    SYMBOL_FOR_FUNCTION_LENGTH, SYMBOL_FUNCTION_LENGTH, SYMBOL_KEY_FOR_FUNCTION_LENGTH,
    SYMBOL_NAME,
};

impl NativeFunctionKind {
    pub(super) const fn core_length(self) -> Option<f64> {
        if let Some(length) = self.promise_length() {
            return Some(length);
        }
        match self {
            Self::ArrayBuffer => Some(ARRAY_BUFFER_FUNCTION_LENGTH),
            Self::TypedArray(_)
            | Self::AsyncGeneratorNext
            | Self::AsyncGeneratorReturn
            | Self::AsyncGeneratorThrow
            | Self::GeneratorNext
            | Self::GeneratorReturn
            | Self::GeneratorThrow
            | Self::PromiseCombinatorElement { .. } => Some(1.0),
            Self::AsyncFunction => Some(ASYNC_FUNCTION_FUNCTION_LENGTH),
            Self::AsyncGeneratorFunction => Some(ASYNC_GENERATOR_FUNCTION_FUNCTION_LENGTH),
            Self::Boolean => Some(BOOLEAN_FUNCTION_LENGTH),
            Self::BigInt => Some(BIGINT_FUNCTION_LENGTH),
            Self::BigIntAsIntN | Self::BigIntAsUintN => Some(2.0),
            Self::BoundFunction(_) => Some(BOUND_FUNCTION_LENGTH),
            Self::Eval => Some(EVAL_FUNCTION_LENGTH),
            Self::ErrorConstructor(name) => Some(name.constructor_length()),
            Self::ErrorPrototypeToString => Some(ERROR_PROTOTYPE_TO_STRING_LENGTH),
            Self::Function => Some(FUNCTION_FUNCTION_LENGTH),
            Self::FunctionPrototypeBind => Some(FUNCTION_PROTOTYPE_BIND_LENGTH),
            Self::FunctionPrototypeCall => Some(FUNCTION_PROTOTYPE_CALL_LENGTH),
            Self::FunctionPrototypeApply => Some(FUNCTION_PROTOTYPE_APPLY_LENGTH),
            Self::FunctionPrototypeHasInstance => Some(FUNCTION_PROTOTYPE_HAS_INSTANCE_LENGTH),
            Self::FunctionPrototypeToString => Some(FUNCTION_PROTOTYPE_TO_STRING_LENGTH),
            Self::JsonIsRawJson => Some(JSON_IS_RAW_JSON_FUNCTION_LENGTH),
            Self::JsonParse => Some(JSON_PARSE_FUNCTION_LENGTH),
            Self::JsonRawJson => Some(JSON_RAW_JSON_FUNCTION_LENGTH),
            Self::JsonStringify => Some(JSON_STRINGIFY_FUNCTION_LENGTH),
            Self::Number => Some(NUMBER_FUNCTION_LENGTH),
            Self::Print | Self::ThrowTypeError | Self::TypedArrayIntrinsic => Some(0.0),
            Self::Proxy => Some(PROXY_FUNCTION_LENGTH),
            Self::ProxyRevocable => Some(PROXY_REVOCABLE_FUNCTION_LENGTH),
            Self::ProxyRevoke(_) => Some(PROXY_REVOKE_FUNCTION_LENGTH),
            Self::SpeciesGetter => Some(SPECIES_GETTER_FUNCTION_LENGTH),
            Self::String => Some(STRING_FUNCTION_LENGTH),
            Self::Symbol => Some(SYMBOL_FUNCTION_LENGTH),
            Self::SymbolFor => Some(SYMBOL_FOR_FUNCTION_LENGTH),
            Self::SymbolKeyFor => Some(SYMBOL_KEY_FOR_FUNCTION_LENGTH),
            _ => None,
        }
    }

    pub(super) const fn core_name(self) -> Option<&'static str> {
        if let Some(name) = self.promise_name() {
            return Some(name);
        }
        match self {
            Self::AsyncFunction => Some(ASYNC_FUNCTION_NAME),
            Self::AsyncGeneratorFunction => Some(ASYNC_GENERATOR_FUNCTION_NAME),
            Self::AsyncGeneratorNext | Self::GeneratorNext => Some("next"),
            Self::AsyncGeneratorReturn | Self::GeneratorReturn => Some("return"),
            Self::AsyncGeneratorThrow | Self::GeneratorThrow => Some("throw"),
            Self::ArrayBuffer => Some(ARRAY_BUFFER_NAME),
            Self::Boolean => Some(BOOLEAN_NAME),
            Self::BigInt => Some(BIGINT_NAME),
            Self::BigIntAsIntN => Some(BIGINT_AS_INT_N_NAME),
            Self::BigIntAsUintN => Some(BIGINT_AS_UINT_N_NAME),
            Self::BoundFunction(_) => Some(BOUND_FUNCTION_NAME),
            Self::Eval => Some(EVAL_NAME),
            Self::ErrorConstructor(name) => Some(name.as_str()),
            Self::ErrorPrototypeToString => Some(ERROR_PROTOTYPE_TO_STRING_NAME),
            Self::Function => Some(FUNCTION_NAME),
            Self::FunctionPrototypeBind => Some(FUNCTION_PROTOTYPE_BIND_NAME),
            Self::FunctionPrototypeCall => Some(FUNCTION_PROTOTYPE_CALL_NAME),
            Self::FunctionPrototypeApply => Some(FUNCTION_PROTOTYPE_APPLY_NAME),
            Self::FunctionPrototypeHasInstance => Some(FUNCTION_PROTOTYPE_HAS_INSTANCE_NAME),
            Self::FunctionPrototypeToString => Some(FUNCTION_PROTOTYPE_TO_STRING_NAME),
            Self::ThrowTypeError | Self::PromiseCombinatorElement { .. } => Some(""),
            Self::JsonIsRawJson => Some(JSON_IS_RAW_JSON_NAME),
            Self::JsonParse => Some(JSON_PARSE_NAME),
            Self::JsonRawJson => Some(JSON_RAW_JSON_NAME),
            Self::JsonStringify => Some(JSON_STRINGIFY_NAME),
            Self::Number => Some(NUMBER_NAME),
            Self::Print => Some("print"),
            Self::Proxy => Some(PROXY_NAME),
            Self::ProxyRevocable => Some(PROXY_REVOCABLE_NAME),
            Self::ProxyRevoke(_) => Some(PROXY_REVOKE_NAME),
            Self::SpeciesGetter => Some(SPECIES_GETTER_NAME),
            Self::String => Some(STRING_NAME),
            Self::Symbol => Some(SYMBOL_NAME),
            Self::SymbolFor => Some("for"),
            Self::SymbolKeyFor => Some("keyFor"),
            Self::TypedArrayIntrinsic => Some("TypedArray"),
            Self::TypedArray(kind) => Some(kind.name()),
            _ => None,
        }
    }
}
