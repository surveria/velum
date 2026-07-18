use core::ops::Range;

/// One optional explicit capture using UTF-16 code-unit offsets.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Capture {
    pub span: Option<Range<usize>>,
}

/// A successful match and its explicit capture groups.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Match {
    pub span: Range<usize>,
    pub captures: Vec<Capture>,
}

/// Bounded-work observations for one complete search.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct ExecutionStats {
    pub steps: usize,
    pub candidate_starts: usize,
    pub max_backtrack_depth: usize,
    pub max_undo_depth: usize,
}

/// Search result plus deterministic resource observations.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchOutcome {
    pub matched: Option<Match>,
    pub stats: ExecutionStats,
}
