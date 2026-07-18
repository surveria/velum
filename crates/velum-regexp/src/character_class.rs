use core::{cmp::Ordering, mem::size_of};

use crate::SizeOverflow;

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
    },
}

impl CharacterClassTerm {
    fn matches(&self, value: u32) -> bool {
        match self {
            Self::Range { start, end } => (*start..=*end).contains(&value),
            Self::StaticRanges { ranges, inverted } => contains(ranges, value) != *inverted,
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
    pub fn matches(&self, value: u32) -> bool {
        self.terms.iter().any(|term| term.matches(value)) != self.inverted
    }

    pub fn retained_payload_bytes(&self) -> Result<usize, SizeOverflow> {
        self.terms
            .len()
            .checked_mul(size_of::<CharacterClassTerm>())
            .ok_or(SizeOverflow)
    }
}

#[must_use]
pub fn is_word_character(value: u32) -> bool {
    contains(WORD_RANGES, value)
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
