const DEFAULT_MAX_SOURCE_LEN: usize = 65_536;
const DEFAULT_MAX_STATEMENTS: usize = 4_096;
const DEFAULT_MAX_EXPRESSION_DEPTH: usize = 256;
const DEFAULT_MAX_RUNTIME_STEPS: usize = 100_000;
const DEFAULT_MAX_STRING_LEN: usize = 65_536;
const DEFAULT_MAX_BINDINGS: usize = 4_096;
const DEFAULT_MAX_OBJECTS: usize = 4_096;
const DEFAULT_MAX_OBJECT_PROPERTIES: usize = 4_096;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub max_source_len: usize,
    pub max_statements: usize,
    pub max_expression_depth: usize,
    pub max_runtime_steps: usize,
    pub max_string_len: usize,
    pub max_bindings: usize,
    pub max_objects: usize,
    pub max_object_properties: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_source_len: DEFAULT_MAX_SOURCE_LEN,
            max_statements: DEFAULT_MAX_STATEMENTS,
            max_expression_depth: DEFAULT_MAX_EXPRESSION_DEPTH,
            max_runtime_steps: DEFAULT_MAX_RUNTIME_STEPS,
            max_string_len: DEFAULT_MAX_STRING_LEN,
            max_bindings: DEFAULT_MAX_BINDINGS,
            max_objects: DEFAULT_MAX_OBJECTS,
            max_object_properties: DEFAULT_MAX_OBJECT_PROPERTIES,
        }
    }
}
