#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegExpValue {
    pattern: String,
    flags: String,
}

impl RegExpValue {
    pub const fn new(pattern: String, flags: String) -> Self {
        Self { pattern, flags }
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn flags(&self) -> &str {
        &self.flags
    }
}
