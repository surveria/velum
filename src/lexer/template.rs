/// Tracks one open `${` substitution while its expression is scanned.
#[derive(Debug)]
pub(super) struct TemplateSubstitutionState {
    pub(super) open_braces: usize,
    pub(super) substitution_offset: usize,
}

/// Identifies the leading or continuation portion of a template literal.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum TemplatePartPosition {
    Head,
    Continuation,
}
