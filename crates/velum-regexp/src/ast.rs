#[derive(Debug, Clone)]
pub enum Node {
    Empty,
    Literal(u32),
    Any,
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
}
