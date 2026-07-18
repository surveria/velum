use core::{cmp::Ordering, mem::size_of};

use crate::{
    Flags, SizeOverflow,
    unicode::{case_closure_all_in_ranges, case_closure_contains},
};

pub const DIGIT_RANGES: &[(u32, u32)] = &[(0x0030, 0x0039)];
pub const WORD_RANGES: &[(u32, u32)] = &[
    (0x0030, 0x0039),
    (0x0041, 0x005A),
    (0x005F, 0x005F),
    (0x0061, 0x007A),
];
pub const SPACE_RANGES: &[(u32, u32)] = &[
    (0x0009, 0x000D),
    (0x0020, 0x0020),
    (0x00A0, 0x00A0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
    (0xFEFF, 0xFEFF),
];

#[derive(Debug, Clone)]
pub enum CharacterClassTerm {
    Range {
        start: u32,
        end: u32,
    },
    StaticRanges {
        ranges: &'static [(u32, u32)],
        inverted: bool,
        complement_before_case_fold: bool,
    },
}

impl CharacterClassTerm {
    fn matches(&self, value: u32, flags: Flags) -> bool {
        match self {
            Self::Range { start, end } => {
                let ranges = [(*start, *end)];
                if flags.ignore_case() {
                    case_closure_contains(&ranges, value, flags.has_unicode_mode())
                } else {
                    (*start..=*end).contains(&value)
                }
            }
            Self::StaticRanges {
                ranges,
                inverted,
                complement_before_case_fold,
            } => {
                let matched = if flags.ignore_case() {
                    case_closure_contains(ranges, value, flags.has_unicode_mode())
                } else {
                    contains(ranges, value)
                };
                if *inverted
                    && *complement_before_case_fold
                    && flags.ignore_case()
                    && flags.unicode()
                {
                    !case_closure_all_in_ranges(ranges, value)
                } else {
                    matched != *inverted
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CharacterClass {
    pub inverted: bool,
    pub terms: Vec<CharacterClassTerm>,
}

impl CharacterClass {
    #[must_use]
    pub fn matches(&self, value: u32, flags: Flags) -> bool {
        self.terms.iter().any(|term| term.matches(value, flags)) != self.inverted
    }

    pub fn retained_payload_bytes(&self) -> Result<usize, SizeOverflow> {
        self.terms
            .len()
            .checked_mul(size_of::<CharacterClassTerm>())
            .ok_or(SizeOverflow)
    }
}

#[must_use]
pub fn is_word_character(value: u32, flags: Flags) -> bool {
    if flags.ignore_case() {
        case_closure_contains(WORD_RANGES, value, flags.has_unicode_mode())
    } else {
        contains(WORD_RANGES, value)
    }
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
