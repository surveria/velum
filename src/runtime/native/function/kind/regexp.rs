use super::NativeFunctionKind;

const REGEXP_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const REGEXP_NAME: &str = "RegExp";
const REGEXP_LEGACY_GETTER_LENGTH: f64 = 0.0;
const REGEXP_LEGACY_INPUT_SETTER_LENGTH: f64 = 1.0;
const REGEXP_LEGACY_INPUT_SETTER_NAME: &str = "set input";
const REGEXP_ESCAPE_LENGTH: f64 = 1.0;
const REGEXP_ESCAPE_NAME: &str = "escape";
const REGEXP_PROTOTYPE_COMPILE_LENGTH: f64 = 2.0;
const REGEXP_PROTOTYPE_COMPILE_NAME: &str = "compile";
const REGEXP_PROTOTYPE_GETTER_LENGTH: f64 = 0.0;
const REGEXP_PROTOTYPE_DOT_ALL_GETTER_NAME: &str = "get dotAll";
pub(in crate::runtime::native) const REGEXP_PROTOTYPE_EXEC_NAME: &str = "exec";
const REGEXP_PROTOTYPE_FLAGS_GETTER_NAME: &str = "get flags";
const REGEXP_PROTOTYPE_GLOBAL_GETTER_NAME: &str = "get global";
const REGEXP_PROTOTYPE_HAS_INDICES_GETTER_NAME: &str = "get hasIndices";
const REGEXP_PROTOTYPE_IGNORE_CASE_GETTER_NAME: &str = "get ignoreCase";
const REGEXP_PROTOTYPE_MULTILINE_GETTER_NAME: &str = "get multiline";
const REGEXP_PROTOTYPE_SOURCE_GETTER_NAME: &str = "get source";
const REGEXP_PROTOTYPE_STICKY_GETTER_NAME: &str = "get sticky";
const REGEXP_PROTOTYPE_TEST_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const REGEXP_PROTOTYPE_TEST_NAME: &str = "test";
const REGEXP_PROTOTYPE_TO_STRING_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const REGEXP_PROTOTYPE_TO_STRING_NAME: &str = "toString";
const REGEXP_PROTOTYPE_UNICODE_GETTER_NAME: &str = "get unicode";
const REGEXP_PROTOTYPE_UNICODE_SETS_GETTER_NAME: &str = "get unicodeSets";
const REGEXP_SYMBOL_METHOD_LENGTH: f64 = 1.0;
const REGEXP_SYMBOL_MATCH_NAME: &str = "[Symbol.match]";
const REGEXP_SYMBOL_MATCH_ALL_NAME: &str = "[Symbol.matchAll]";
const REGEXP_SYMBOL_REPLACE_LENGTH: f64 = 2.0;
const REGEXP_SYMBOL_REPLACE_NAME: &str = "[Symbol.replace]";
const REGEXP_SYMBOL_SEARCH_NAME: &str = "[Symbol.search]";
const REGEXP_SYMBOL_SPLIT_LENGTH: f64 = 2.0;
const REGEXP_SYMBOL_SPLIT_NAME: &str = "[Symbol.split]";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum LegacyRegExpStaticKind {
    Input,
    LastMatch,
    LastParen,
    LeftContext,
    RightContext,
    Capture(u8),
}

impl LegacyRegExpStaticKind {
    const fn getter_name(self) -> &'static str {
        match self {
            Self::Input => "get input",
            Self::LastMatch => "get lastMatch",
            Self::LastParen => "get lastParen",
            Self::LeftContext => "get leftContext",
            Self::RightContext => "get rightContext",
            Self::Capture(1) => "get $1",
            Self::Capture(2) => "get $2",
            Self::Capture(3) => "get $3",
            Self::Capture(4) => "get $4",
            Self::Capture(5) => "get $5",
            Self::Capture(6) => "get $6",
            Self::Capture(7) => "get $7",
            Self::Capture(8) => "get $8",
            Self::Capture(9) => "get $9",
            Self::Capture(_) => "get legacy capture",
        }
    }
}

impl NativeFunctionKind {
    pub(super) const fn regexp_length(self) -> Option<f64> {
        match self {
            Self::RegExp => Some(REGEXP_FUNCTION_LENGTH),
            Self::RegExpEscape => Some(REGEXP_ESCAPE_LENGTH),
            Self::RegExpLegacyGetter(_) => Some(REGEXP_LEGACY_GETTER_LENGTH),
            Self::RegExpLegacyInputSetter => Some(REGEXP_LEGACY_INPUT_SETTER_LENGTH),
            Self::RegExpPrototypeCompile => Some(REGEXP_PROTOTYPE_COMPILE_LENGTH),
            Self::RegExpPrototypeDotAllGetter
            | Self::RegExpPrototypeFlagsGetter
            | Self::RegExpPrototypeGlobalGetter
            | Self::RegExpPrototypeHasIndicesGetter
            | Self::RegExpPrototypeIgnoreCaseGetter
            | Self::RegExpPrototypeMultilineGetter
            | Self::RegExpPrototypeSourceGetter
            | Self::RegExpPrototypeStickyGetter
            | Self::RegExpPrototypeUnicodeGetter
            | Self::RegExpPrototypeUnicodeSetsGetter => Some(REGEXP_PROTOTYPE_GETTER_LENGTH),
            Self::RegExpPrototypeExec | Self::RegExpPrototypeTest => {
                Some(REGEXP_PROTOTYPE_TEST_LENGTH)
            }
            Self::RegExpPrototypeToString => Some(REGEXP_PROTOTYPE_TO_STRING_LENGTH),
            Self::RegExpPrototypeSymbolMatch
            | Self::RegExpPrototypeSymbolMatchAll
            | Self::RegExpPrototypeSymbolSearch => Some(REGEXP_SYMBOL_METHOD_LENGTH),
            Self::RegExpPrototypeSymbolReplace => Some(REGEXP_SYMBOL_REPLACE_LENGTH),
            Self::RegExpPrototypeSymbolSplit => Some(REGEXP_SYMBOL_SPLIT_LENGTH),
            _ => None,
        }
    }

    pub(super) const fn regexp_name(self) -> Option<&'static str> {
        match self {
            Self::RegExp => Some(REGEXP_NAME),
            Self::RegExpEscape => Some(REGEXP_ESCAPE_NAME),
            Self::RegExpLegacyGetter(kind) => Some(kind.getter_name()),
            Self::RegExpLegacyInputSetter => Some(REGEXP_LEGACY_INPUT_SETTER_NAME),
            Self::RegExpPrototypeCompile => Some(REGEXP_PROTOTYPE_COMPILE_NAME),
            Self::RegExpPrototypeDotAllGetter => Some(REGEXP_PROTOTYPE_DOT_ALL_GETTER_NAME),
            Self::RegExpPrototypeExec => Some(REGEXP_PROTOTYPE_EXEC_NAME),
            Self::RegExpPrototypeFlagsGetter => Some(REGEXP_PROTOTYPE_FLAGS_GETTER_NAME),
            Self::RegExpPrototypeGlobalGetter => Some(REGEXP_PROTOTYPE_GLOBAL_GETTER_NAME),
            Self::RegExpPrototypeHasIndicesGetter => Some(REGEXP_PROTOTYPE_HAS_INDICES_GETTER_NAME),
            Self::RegExpPrototypeIgnoreCaseGetter => Some(REGEXP_PROTOTYPE_IGNORE_CASE_GETTER_NAME),
            Self::RegExpPrototypeMultilineGetter => Some(REGEXP_PROTOTYPE_MULTILINE_GETTER_NAME),
            Self::RegExpPrototypeSourceGetter => Some(REGEXP_PROTOTYPE_SOURCE_GETTER_NAME),
            Self::RegExpPrototypeStickyGetter => Some(REGEXP_PROTOTYPE_STICKY_GETTER_NAME),
            Self::RegExpPrototypeTest => Some(REGEXP_PROTOTYPE_TEST_NAME),
            Self::RegExpPrototypeToString => Some(REGEXP_PROTOTYPE_TO_STRING_NAME),
            Self::RegExpPrototypeUnicodeGetter => Some(REGEXP_PROTOTYPE_UNICODE_GETTER_NAME),
            Self::RegExpPrototypeUnicodeSetsGetter => {
                Some(REGEXP_PROTOTYPE_UNICODE_SETS_GETTER_NAME)
            }
            Self::RegExpPrototypeSymbolMatch => Some(REGEXP_SYMBOL_MATCH_NAME),
            Self::RegExpPrototypeSymbolMatchAll => Some(REGEXP_SYMBOL_MATCH_ALL_NAME),
            Self::RegExpPrototypeSymbolReplace => Some(REGEXP_SYMBOL_REPLACE_NAME),
            Self::RegExpPrototypeSymbolSearch => Some(REGEXP_SYMBOL_SEARCH_NAME),
            Self::RegExpPrototypeSymbolSplit => Some(REGEXP_SYMBOL_SPLIT_NAME),
            _ => None,
        }
    }
}
