use regress::{Flags, Regex};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct RegExpFlags {
    bits: u16,
}

impl RegExpFlags {
    pub(super) fn parse(flags: &str) -> Result<Self, RegExpSyntaxError> {
        let mut seen = Self::default();
        for flag in flags.chars() {
            seen.record(flag)?;
        }
        Ok(seen)
    }

    const fn record(&mut self, flag: char) -> Result<(), RegExpSyntaxError> {
        let bit = match flag {
            'g' => REGEXP_FLAG_GLOBAL,
            'i' => REGEXP_FLAG_IGNORE_CASE,
            'm' => REGEXP_FLAG_MULTILINE,
            's' => REGEXP_FLAG_DOT_ALL,
            'u' => REGEXP_FLAG_UNICODE,
            'y' => REGEXP_FLAG_STICKY,
            'd' => REGEXP_FLAG_HAS_INDICES,
            'v' => REGEXP_FLAG_UNICODE_SETS,
            _ => return Err(RegExpSyntaxError::UnsupportedFlag(flag)),
        };
        if self.bits & bit != 0 {
            return Err(RegExpSyntaxError::DuplicateFlag(flag));
        }
        self.bits |= bit;
        Ok(())
    }

    pub(super) const fn regress_flags(self) -> Flags {
        Flags {
            icase: self.ignore_case(),
            multiline: self.multiline(),
            dot_all: self.dot_all(),
            no_opt: false,
            unicode: self.unicode(),
            unicode_sets: self.unicode_sets(),
        }
    }

    pub(super) const fn ignore_case(self) -> bool {
        self.bits & REGEXP_FLAG_IGNORE_CASE != 0
    }

    pub(super) const fn multiline(self) -> bool {
        self.bits & REGEXP_FLAG_MULTILINE != 0
    }

    pub(super) const fn dot_all(self) -> bool {
        self.bits & REGEXP_FLAG_DOT_ALL != 0
    }

    pub(super) const fn global(self) -> bool {
        self.bits & REGEXP_FLAG_GLOBAL != 0
    }

    pub(super) const fn sticky(self) -> bool {
        self.bits & REGEXP_FLAG_STICKY != 0
    }

    pub(super) const fn has_indices(self) -> bool {
        self.bits & REGEXP_FLAG_HAS_INDICES != 0
    }

    pub(super) const fn unicode(self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE != 0
    }

    pub(super) const fn unicode_sets(self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE_SETS != 0
    }

    pub(super) fn canonical_text(self) -> String {
        let mut flags = String::new();
        for (enabled, flag) in [
            (self.has_indices(), 'd'),
            (self.global(), 'g'),
            (self.ignore_case(), 'i'),
            (self.multiline(), 'm'),
            (self.dot_all(), 's'),
            (self.unicode(), 'u'),
            (self.unicode_sets(), 'v'),
            (self.sticky(), 'y'),
        ] {
            if enabled {
                flags.push(flag);
            }
        }
        flags
    }
}

pub fn compile_regexp(pattern: &str, flags: RegExpFlags) -> Result<Regex, RegExpSyntaxError> {
    let pattern = normalized_regexp_pattern(pattern);
    Regex::with_flags(&pattern, flags.regress_flags())
        .map_err(|error| RegExpSyntaxError::InvalidPattern(error.to_string()))
}

fn normalized_regexp_pattern(pattern: &str) -> String {
    let mut normalized = pattern.to_owned();
    for (source, replacement) in UNKNOWN_SCRIPT_EXTENSIONS_REWRITES {
        normalized = normalized.replace(source, replacement);
    }
    normalized
}

pub fn validate_regexp_literal(pattern: &str, flags: &str) -> Result<(), RegExpSyntaxError> {
    let flags = RegExpFlags::parse(flags)?;
    compile_regexp(pattern, flags).map(drop)
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum RegExpSyntaxError {
    #[error("unsupported regular expression flag: {0}")]
    UnsupportedFlag(char),
    #[error("duplicate regular expression flag: {0}")]
    DuplicateFlag(char),
    #[error("invalid regular expression pattern: {0}")]
    InvalidPattern(String),
}

const REGEXP_FLAG_GLOBAL: u16 = 1 << 0;
const REGEXP_FLAG_IGNORE_CASE: u16 = 1 << 1;
const REGEXP_FLAG_MULTILINE: u16 = 1 << 2;
const REGEXP_FLAG_DOT_ALL: u16 = 1 << 3;
const REGEXP_FLAG_UNICODE: u16 = 1 << 4;
const REGEXP_FLAG_STICKY: u16 = 1 << 5;
const REGEXP_FLAG_HAS_INDICES: u16 = 1 << 6;
const REGEXP_FLAG_UNICODE_SETS: u16 = 1 << 7;

// Unicode Script_Extensions=Unknown contains unassigned, private-use, and
// surrogate code points. Regress 0.11.1 omits the Unknown/Zzzz aliases, so
// spell that standard set through the general categories it already owns.
const UNKNOWN_SCRIPT_EXTENSIONS_CLASS: &str = r"[\p{Cn}\p{Co}\p{Cs}]";
const UNKNOWN_SCRIPT_EXTENSIONS_REWRITES: [(&str, &str); 4] = [
    (
        r"\p{Script_Extensions=Unknown}",
        UNKNOWN_SCRIPT_EXTENSIONS_CLASS,
    ),
    (
        r"\p{Script_Extensions=Zzzz}",
        UNKNOWN_SCRIPT_EXTENSIONS_CLASS,
    ),
    (r"\p{scx=Unknown}", UNKNOWN_SCRIPT_EXTENSIONS_CLASS),
    (r"\p{scx=Zzzz}", UNKNOWN_SCRIPT_EXTENSIONS_CLASS),
];
