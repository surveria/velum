use crate::bytecode::BytecodeProgram;

impl BytecodeProgram {
    pub fn instruction_count(&self) -> usize {
        self.block().instruction_count()
    }

    pub fn binding_operand_count(&self) -> usize {
        self.block().binding_operand_count()
    }

    pub fn property_operand_count(&self) -> usize {
        self.block().property_operand_count()
    }

    pub fn direct_native_call_count(&self) -> usize {
        self.block().direct_native_call_count()
    }

    pub fn array_native_call_count(&self) -> usize {
        self.block().array_native_call_count()
    }

    pub fn numeric_instruction_count(&self) -> usize {
        self.block().numeric_instruction_count()
    }
}
