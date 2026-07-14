use crate::{
    ast::{ClassField, FunctionParam, Statement, StaticBinding},
    binding_metadata::BindingLayout,
    bytecode::{BytecodeEvalMode, BytecodeFunction, BytecodeFunctionInit},
    error::Result,
};

use super::{
    BytecodeBlock, BytecodeHoistPlan, CaptureBindingCollector, FunctionCompileMode, compile_params,
};

impl BytecodeFunction {
    pub(in crate::compiler) fn compile(
        self_binding: Option<StaticBinding>,
        arguments_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        mode: FunctionCompileMode,
        layout: &BindingLayout,
    ) -> Result<Self> {
        Self::compile_with_additional_captures(
            self_binding,
            arguments_binding,
            params,
            statements,
            mode,
            layout,
            std::iter::empty(),
        )
    }

    pub(in crate::compiler) fn compile_class_constructor(
        arguments_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        fields: &[ClassField],
        mode: FunctionCompileMode,
        layout: &BindingLayout,
    ) -> Result<Self> {
        Self::compile_with_additional_captures(
            None,
            arguments_binding,
            params,
            statements,
            mode,
            layout,
            fields
                .iter()
                .filter(|field| !field.is_static)
                .filter_map(|field| field.initializer.as_ref()),
        )
    }

    fn compile_with_additional_captures<'a>(
        self_binding: Option<StaticBinding>,
        arguments_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        mode: FunctionCompileMode,
        layout: &BindingLayout,
        additional_expressions: impl IntoIterator<Item = &'a crate::ast::Expression>,
    ) -> Result<Self> {
        let collected = CaptureBindingCollector::collect_function_with_additional(
            params,
            statements,
            additional_expressions,
        );
        let uses_arguments = collected.uses_arguments || arguments_binding.is_some();
        Ok(Self::new(BytecodeFunctionInit {
            self_binding,
            arguments_binding,
            params: compile_params(params, layout)?,
            body: BytecodeBlock::compile_function_statements(statements, mode.kind, layout)?,
            hoist_plan: BytecodeHoistPlan::compile(statements, layout)?,
            capture_bindings: collected.bindings,
            uses_arguments,
            eval_mode: BytecodeEvalMode::new(mode.strict, collected.contains_direct_eval),
            simple_parameters: params.iter().all(FunctionParam::is_simple_binding),
        }))
    }
}
