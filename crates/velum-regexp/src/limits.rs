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
    pub max_repeat_count: u64,
}

impl Default for CompileLimits {
    fn default() -> Self {
        Self {
            max_pattern_units: 65_536,
            max_repeat_count: 1_000_000,
            ..Self::MAXIMUM
        }
    }
}

impl CompileLimits {
    /// Engine-enforced ceilings for caller-selected compile limits.
    pub const MAXIMUM: Self = Self {
        max_pattern_units: 1_048_576,
        max_nesting_depth: 256,
        max_nodes: 65_536,
        max_instructions: 262_144,
        max_captures: 4_096,
        max_capture_name_units: 1_024,
        max_character_class_terms: 65_536,
        max_class_strings: 16_384,
        max_class_string_units: 131_072,
        max_repeat_count: 9_007_199_254_740_991,
    };

    pub(crate) fn constrained(self) -> Self {
        Self {
            max_pattern_units: self.max_pattern_units.min(Self::MAXIMUM.max_pattern_units),
            max_nesting_depth: self.max_nesting_depth.min(Self::MAXIMUM.max_nesting_depth),
            max_nodes: self.max_nodes.min(Self::MAXIMUM.max_nodes),
            max_instructions: self.max_instructions.min(Self::MAXIMUM.max_instructions),
            max_captures: self.max_captures.min(Self::MAXIMUM.max_captures),
            max_capture_name_units: self
                .max_capture_name_units
                .min(Self::MAXIMUM.max_capture_name_units),
            max_character_class_terms: self
                .max_character_class_terms
                .min(Self::MAXIMUM.max_character_class_terms),
            max_class_strings: self.max_class_strings.min(Self::MAXIMUM.max_class_strings),
            max_class_string_units: self
                .max_class_string_units
                .min(Self::MAXIMUM.max_class_string_units),
            max_repeat_count: self.max_repeat_count.min(Self::MAXIMUM.max_repeat_count),
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
            ..Self::MAXIMUM
        }
    }
}

impl ExecutionLimits {
    /// Engine-enforced ceilings for caller-selected execution limits.
    pub const MAXIMUM: Self = Self {
        max_steps: 268_435_456,
        max_candidate_starts: 33_554_433,
        max_backtrack_frames: 65_536,
        max_undo_records: 262_144,
        max_capture_slots: 4_096,
    };

    pub(crate) fn constrained(self) -> Self {
        Self {
            max_steps: self.max_steps.min(Self::MAXIMUM.max_steps),
            max_candidate_starts: self
                .max_candidate_starts
                .min(Self::MAXIMUM.max_candidate_starts),
            max_backtrack_frames: self
                .max_backtrack_frames
                .min(Self::MAXIMUM.max_backtrack_frames),
            max_undo_records: self.max_undo_records.min(Self::MAXIMUM.max_undo_records),
            max_capture_slots: self.max_capture_slots.min(Self::MAXIMUM.max_capture_slots),
        }
    }
}
