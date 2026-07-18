/// Hard limits applied while parsing and compiling one pattern.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompileLimits {
    pub max_pattern_units: usize,
    pub max_nesting_depth: usize,
    pub max_nodes: usize,
    pub max_instructions: usize,
    pub max_captures: usize,
    pub max_capture_name_units: usize,
    pub max_character_class_terms: usize,
    pub max_class_strings: usize,
    pub max_class_string_units: usize,
    pub max_repeat_count: u32,
}

impl Default for CompileLimits {
    fn default() -> Self {
        Self {
            max_pattern_units: 65_536,
            max_nesting_depth: 256,
            max_nodes: 65_536,
            max_instructions: 262_144,
            max_captures: 4_096,
            max_capture_name_units: 1_024,
            max_character_class_terms: 65_536,
            max_class_strings: 16_384,
            max_class_string_units: 131_072,
            max_repeat_count: 1_000_000,
        }
    }
}

/// Hard limits applied across one complete search operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ExecutionLimits {
    pub max_steps: usize,
    pub max_candidate_starts: usize,
    pub max_backtrack_frames: usize,
    pub max_undo_records: usize,
    pub max_capture_slots: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_steps: 1_000_000,
            max_candidate_starts: 65_537,
            max_backtrack_frames: 65_536,
            max_undo_records: 262_144,
            max_capture_slots: 4_096,
        }
    }
}
