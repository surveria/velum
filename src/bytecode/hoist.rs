use std::rc::Rc;

use crate::{bytecode::BytecodeFunctionDeclaration, syntax::StaticBinding};

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeHoistPlan {
    var_declarations: Rc<[StaticBinding]>,
    function_declarations: Rc<[BytecodeFunctionDeclaration]>,
}

impl BytecodeHoistPlan {
    pub(crate) const fn new(
        var_declarations: Rc<[StaticBinding]>,
        function_declarations: Rc<[BytecodeFunctionDeclaration]>,
    ) -> Self {
        Self {
            var_declarations,
            function_declarations,
        }
    }

    pub fn var_declarations(&self) -> &[StaticBinding] {
        &self.var_declarations
    }

    pub fn function_declarations(&self) -> &[BytecodeFunctionDeclaration] {
        &self.function_declarations
    }

    pub fn var_declaration_count(&self) -> usize {
        self.var_declarations.len()
    }

    pub fn function_declaration_count(&self) -> usize {
        self.function_declarations.len()
    }
}
