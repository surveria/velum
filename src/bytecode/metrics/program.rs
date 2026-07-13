use crate::bytecode::{BytecodeMetrics, BytecodeProgram};

impl BytecodeProgram {
    pub(crate) fn metrics(&self) -> BytecodeMetrics {
        self.block().metrics().combine(self.hoist_plan().metrics())
    }
}
