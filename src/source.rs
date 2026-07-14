use std::fmt;

const FNV_1A_128_OFFSET_BASIS: u128 = 144_066_263_297_769_815_596_495_629_667_062_367_629;
const FNV_1A_128_PRIME: u128 = 309_485_009_821_345_068_724_781_371;
const SOURCE_ID_DOMAIN: &[u8] = b"rs-quickjs-source-id-v1";
const ANONYMOUS_SOURCE_TAG: u8 = 0;
const NAMED_SOURCE_TAG: u8 = 1;
const SOURCE_TEXT_TAG: u8 = 2;
const SOURCE_UTF16_TEXT_TAG: u8 = 3;
const STABLE_LENGTH_BYTES: usize = 16;

/// A stable diagnostic identity for JavaScript source text.
///
/// The identity is derived from the optional source name and source bytes. It
/// is stable across VMs and repeated compilation, but it is not a security or
/// ownership boundary. VM-owned values require separate handle validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(u128);

impl SourceId {
    pub(crate) const UNKNOWN: Self = Self(0);

    /// Derives the identity used by anonymous compilation.
    #[must_use]
    pub fn for_source(source: &str) -> Self {
        Self::for_optional_name(None, source)
    }

    /// Derives the identity used by named compilation.
    #[must_use]
    pub fn for_named_source(name: &str, source: &str) -> Self {
        Self::for_optional_name(Some(name), source)
    }

    /// Returns the deterministic numeric representation of this identity.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    pub(crate) fn for_optional_name(name: Option<&str>, source: &str) -> Self {
        let mut hash = fnv_1a_128(FNV_1A_128_OFFSET_BASIS, SOURCE_ID_DOMAIN);
        if let Some(name) = name {
            hash = fnv_1a_128(hash, &[NAMED_SOURCE_TAG]);
            hash = fnv_1a_128_usize(hash, name.len());
            hash = fnv_1a_128(hash, name.as_bytes());
        } else {
            hash = fnv_1a_128(hash, &[ANONYMOUS_SOURCE_TAG]);
        }
        hash = fnv_1a_128(hash, &[SOURCE_TEXT_TAG]);
        hash = fnv_1a_128_usize(hash, source.len());
        hash = fnv_1a_128(hash, source.as_bytes());
        if hash == Self::UNKNOWN.0 {
            return Self(1);
        }
        Self(hash)
    }

    pub(crate) fn for_optional_name_utf16(name: Option<&str>, source: &[u16]) -> Self {
        let mut hash = fnv_1a_128(FNV_1A_128_OFFSET_BASIS, SOURCE_ID_DOMAIN);
        if let Some(name) = name {
            hash = fnv_1a_128(hash, &[NAMED_SOURCE_TAG]);
            hash = fnv_1a_128_usize(hash, name.len());
            hash = fnv_1a_128(hash, name.as_bytes());
        } else {
            hash = fnv_1a_128(hash, &[ANONYMOUS_SOURCE_TAG]);
        }
        hash = fnv_1a_128(hash, &[SOURCE_UTF16_TEXT_TAG]);
        hash = fnv_1a_128_usize(hash, source.len());
        for unit in source {
            hash = fnv_1a_128(hash, &unit.to_le_bytes());
        }
        if hash == Self::UNKNOWN.0 {
            return Self(1);
        }
        Self(hash)
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:032x}", self.0)
    }
}

/// A half-open byte range in one JavaScript source.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    source_id: SourceId,
    start: usize,
    end: usize,
}

impl SourceSpan {
    /// Creates a point span at a byte offset.
    #[must_use]
    pub const fn point(source_id: SourceId, offset: usize) -> Self {
        Self {
            source_id,
            start: offset,
            end: offset,
        }
    }

    /// Creates a half-open span when `start <= end`.
    #[must_use]
    pub const fn new(source_id: SourceId, start: usize, end: usize) -> Option<Self> {
        if start > end {
            return None;
        }
        Some(Self {
            source_id,
            start,
            end,
        })
    }

    pub(crate) const fn from_valid_bounds(source_id: SourceId, start: usize, end: usize) -> Self {
        Self {
            source_id,
            start,
            end,
        }
    }

    /// Returns the source identity owning this range.
    #[must_use]
    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    /// Returns the inclusive start byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the exclusive end byte offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns whether this range points between source bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Returns the smallest range covering both spans when they share a source.
    #[must_use]
    pub const fn cover(self, other: Self) -> Option<Self> {
        if self.source_id.as_u128() != other.source_id.as_u128() {
            return None;
        }
        Some(Self {
            source_id: self.source_id,
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        })
    }

    pub(crate) fn for_diagnostic(source_id: SourceId, source: &str, offset: usize) -> Self {
        let start = offset.min(source.len());
        let Some(remainder) = source.get(start..) else {
            return Self::point(source_id, start);
        };
        let Some(character) = remainder.chars().next() else {
            return Self::point(source_id, start);
        };
        let Some(end) = start.checked_add(character.len_utf8()) else {
            return Self::point(source_id, start);
        };
        Self {
            source_id,
            start,
            end,
        }
    }
}

fn fnv_1a_128_usize(mut hash: u128, value: usize) -> u128 {
    let bytes = value.to_le_bytes();
    hash = fnv_1a_128(hash, &bytes);
    for _ in bytes.len()..STABLE_LENGTH_BYTES {
        hash = fnv_1a_128(hash, &[0]);
    }
    hash
}

fn fnv_1a_128(mut hash: u128, bytes: &[u8]) -> u128 {
    for byte in bytes {
        hash ^= u128::from(*byte);
        let (next, _overflowed) = hash.overflowing_mul(FNV_1A_128_PRIME);
        // FNV-1a is defined with modular arithmetic, so overflow is intentional.
        hash = next;
    }
    hash
}
