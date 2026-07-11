#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegExpValue {
    pattern: String,
    pattern_units: Vec<u16>,
    flags: String,
}

impl RegExpValue {
    pub fn new_utf16(pattern_units: Vec<u16>, flags: String) -> Self {
        let pattern = String::from_utf16_lossy(&pattern_units);
        Self {
            pattern,
            pattern_units,
            flags,
        }
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn pattern_utf16(&self) -> &[u16] {
        &self.pattern_units
    }

    pub fn flags(&self) -> &str {
        &self.flags
    }
}
