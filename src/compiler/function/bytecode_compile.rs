use crate::{
    ast::{ClassField, FunctionParam, Statement, StaticBinding},
    binding_metadata::BindingLayout,
    bytecode::{BytecodeEvalMode, BytecodeFunction, BytecodeFunctionInit},
    error::Result,
};

use super::{
    BytecodeBlock, BytecodeHoistPlan, CaptureBindingCollector, FunctionCompileMode, compile_params,
};

struct FunctionCompileInput<'a> {
    self_binding: Option<StaticBinding>,
    arguments_binding: Option<StaticBinding>,
    params: &'a [FunctionParam],
    statements: &'a [Statement],
    mode: FunctionCompileMode,
    layout: &'a BindingLayout,
    source: Option<std::rc::Rc<str>>,
}

impl BytecodeFunction {
    pub(in crate::compiler) fn compile(
        self_binding: Option<StaticBinding>,
        arguments_binding: Option<StaticBinding>,
        params: &[FunctionParam],
        statements: &[Statement],
        mode: FunctionCompileMode,
        layout: &BindingLayout,
        source: Option<std::rc::Rc<str>>,
    ) -> Result<Self> {
        Self::compile_with_additional_captures(
            FunctionCompileInput {
                self_binding,
                arguments_binding,
                params,
                statements,
                mode,
                layout,
                source,
            },
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
        source: Option<std::rc::Rc<str>>,
    ) -> Result<Self> {
        Self::compile_with_additional_captures(
            FunctionCompileInput {
                self_binding: None,
                arguments_binding,
                params,
                statements,
                mode,
                layout,
                source,
            },
            fields
                .iter()
                .filter(|field| !field.is_static)
                .filter_map(|field| field.initializer.as_ref()),
        )
    }

    fn compile_with_additional_captures<'a>(
        input: FunctionCompileInput<'_>,
        additional_expressions: impl IntoIterator<Item = &'a crate::ast::Expression>,
    ) -> Result<Self> {
        let FunctionCompileInput {
            self_binding,
            arguments_binding,
            params,
            statements,
            mode,
            layout,
            source,
        } = input;
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
            contains_direct_eval: collected.contains_direct_eval,
            requires_dynamic_lexical_capture: collected.requires_dynamic_lexical_capture,
            eval_mode: BytecodeEvalMode::new(mode.strict, collected.contains_direct_eval),
            simple_parameters: params.iter().all(FunctionParam::is_simple_binding),
            source,
        }))
    }
}
