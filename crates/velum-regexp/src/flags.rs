const IGNORE_CASE: u8 = 1 << 0;
const MULTILINE: u8 = 1 << 1;
const DOT_ALL: u8 = 1 << 2;
const UNICODE: u8 = 1 << 3;
const UNICODE_SETS: u8 = 1 << 4;

/// Compile-time ECMAScript `RegExp` flags owned by the matching engine.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct Flags(u8);

impl Flags {
    /// Returns true when either Unicode mode is enabled.
    #[must_use]
    pub const fn has_unicode_mode(self) -> bool {
        self.unicode() || self.unicode_sets()
    }

    #[must_use]
    pub const fn with_ignore_case(self, enabled: bool) -> Self {
        self.with_bit(IGNORE_CASE, enabled)
    }

    #[must_use]
    pub const fn with_multiline(self, enabled: bool) -> Self {
        self.with_bit(MULTILINE, enabled)
    }

    #[must_use]
    pub const fn with_dot_all(self, enabled: bool) -> Self {
        self.with_bit(DOT_ALL, enabled)
    }

    #[must_use]
    pub const fn with_unicode(self, enabled: bool) -> Self {
        self.with_bit(UNICODE, enabled)
    }

    #[must_use]
    pub const fn with_unicode_sets(self, enabled: bool) -> Self {
        self.with_bit(UNICODE_SETS, enabled)
    }

    #[must_use]
    pub const fn ignore_case(self) -> bool {
        self.0 & IGNORE_CASE != 0
    }

    #[must_use]
    pub const fn multiline(self) -> bool {
        self.0 & MULTILINE != 0
    }

    #[must_use]
    pub const fn dot_all(self) -> bool {
        self.0 & DOT_ALL != 0
    }

    #[must_use]
    pub const fn unicode(self) -> bool {
        self.0 & UNICODE != 0
    }

    #[must_use]
    pub const fn unicode_sets(self) -> bool {
        self.0 & UNICODE_SETS != 0
    }

    pub(crate) const fn apply_modifiers(self, set: Self, unset: Self) -> Self {
        Self((self.0 | set.0) & !unset.0)
    }

    const fn with_bit(self, bit: u8, enabled: bool) -> Self {
        if enabled {
            Self(self.0 | bit)
        } else {
            Self(self.0 & !bit)
        }
    }
}
