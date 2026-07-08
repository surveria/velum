use crate::{ast::Program, binding_metadata::BindingLayout, error::Result};

use super::LayoutBuilder;

impl BindingLayout {
    pub fn build(
        program: &Program,
        static_binding_count: usize,
        static_function_count: usize,
    ) -> Result<Self> {
        let mut builder = LayoutBuilder::new(static_binding_count, static_function_count);
        builder.build(program)
    }
}
