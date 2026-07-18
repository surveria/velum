use crate::{Flags, character_class::CharacterClass};

#[derive(Debug, Clone)]
pub enum Node {
    Empty,
    Literal(u32),
    Backreference {
        id: usize,
        pattern_offset: usize,
    },
    NamedBackreference {
        name: String,
        pattern_offset: usize,
    },
    BackreferenceSet {
        ids: Vec<usize>,
    },
    Class(CharacterClass),
    Any,
    WordBoundary(bool),
    Lookahead {
        body: Box<Self>,
        positive: bool,
    },
    Lookbehind {
        body: Box<Self>,
        positive: bool,
    },
    Modifier {
        body: Box<Self>,
        set: Flags,
        unset: Flags,
    },
    AssertStart,
    AssertEnd,
    Concat(Vec<Self>),
    LegacySequence(Vec<Self>),
    Alternation(Vec<Self>),
    Capture {
        id: usize,
        body: Box<Self>,
    },
    Repeat {
        body: Box<Self>,
        min: u64,
        max: Option<u64>,
        greedy: bool,
    },
}

#[derive(Debug)]
pub struct ParsedPattern {
    pub root: Node,
    pub capture_count: usize,
    pub capture_names: Vec<Option<String>>,
}
