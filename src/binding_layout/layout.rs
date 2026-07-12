use crate::{ast::Program, binding_metadata::BindingLayout, error::Result};

use super::{LayoutBuilder, builder::RootLayoutMode};

impl BindingLayout {
    pub fn build(
        program: &Program,
        static_binding_count: usize,
        static_function_count: usize,
    ) -> Result<Self> {
        let mut builder = LayoutBuilder::new(static_binding_count, static_function_count);
        builder.build(program, &RootLayoutMode::Script)
    }

    pub(crate) fn build_eval(
        program: &Program,
        static_binding_count: usize,
        static_function_count: usize,
        strict: bool,
    ) -> Result<Self> {
        let mode = if strict {
            RootLayoutMode::StrictEval
        } else {
            RootLayoutMode::SloppyEval
        };
        let mut builder = LayoutBuilder::new(static_binding_count, static_function_count);
        builder.build(program, &mode)
    }

    pub(crate) fn build_module(
        program: &Program,
        static_binding_count: usize,
        static_function_count: usize,
    ) -> Result<Self> {
        let mut builder = LayoutBuilder::new(static_binding_count, static_function_count);
        builder.build(program, &RootLayoutMode::StrictEval)
    }
}
