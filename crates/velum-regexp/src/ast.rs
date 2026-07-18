use crate::character_class::CharacterClass;

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
    AssertStart,
    AssertEnd,
    Concat(Vec<Self>),
    Alternation(Vec<Self>),
    Capture {
        id: usize,
        body: Box<Self>,
    },
    Repeat {
        body: Box<Self>,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    },
}

#[derive(Debug)]
pub struct ParsedPattern {
    pub root: Node,
    pub capture_count: usize,
    pub capture_names: Vec<Option<String>>,
}
