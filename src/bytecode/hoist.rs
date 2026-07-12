use std::rc::Rc;

use crate::{
    bytecode::BytecodeFunctionDeclaration,
    syntax::{DeclKind, StaticBinding},
};

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeHoistPlan {
    lexical: Rc<[(StaticBinding, DeclKind)]>,
    vars: Rc<[StaticBinding]>,
    functions: Rc<[BytecodeFunctionDeclaration]>,
}

impl BytecodeHoistPlan {
    pub(crate) const fn new(
        lexical_declarations: Rc<[(StaticBinding, DeclKind)]>,
        var_declarations: Rc<[StaticBinding]>,
        function_declarations: Rc<[BytecodeFunctionDeclaration]>,
    ) -> Self {
        Self {
            lexical: lexical_declarations,
            vars: var_declarations,
            functions: function_declarations,
        }
    }

    pub fn lexical_declarations(&self) -> &[(StaticBinding, DeclKind)] {
        &self.lexical
    }

    pub fn var_declarations(&self) -> &[StaticBinding] {
        &self.vars
    }

    pub fn function_declarations(&self) -> &[BytecodeFunctionDeclaration] {
        &self.functions
    }

    pub fn var_declaration_count(&self) -> usize {
        self.vars.len()
    }

    pub fn lexical_declaration_count(&self) -> usize {
        self.lexical.len()
    }

    pub fn function_declaration_count(&self) -> usize {
        self.functions.len()
    }
}
