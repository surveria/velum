mod usage;

pub use usage::CompiledScriptUsage;

use crate::{
    binding_layout::BindingLayout,
    bytecode::BytecodeProgram,
    error::{Error, Result},
    lexer, parser,
    runtime::limits::RuntimeLimits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScript {
    bytecode: BytecodeProgram,
    binding_layout: BindingLayout,
    usage: CompiledScriptUsage,
}

impl CompiledScript {
    pub(crate) fn compile(source: &str, limits: RuntimeLimits) -> Result<Self> {
        check_source_len(source, limits)?;
        let tokens = lexer::lex(source)?;
        let parsed = parser::parse_with_usage(tokens, limits)?;
        let program = parsed.program;
        let binding_layout = BindingLayout::build(
            &program,
            parsed.usage.static_binding_count,
            parsed.usage.static_function_count,
        )?;
        let bytecode = BytecodeProgram::compile(&program, &binding_layout)?;
        let bytecode_instruction_count = bytecode.instruction_count();
        let bytecode_binding_operand_count = bytecode.binding_operand_count();
        let bytecode_property_operand_count = bytecode.property_operand_count();
        let bytecode_direct_native_call_count = bytecode.direct_native_call_count();
        let bytecode_array_native_call_count = bytecode.array_native_call_count();
        let bytecode_numeric_instruction_count = bytecode.numeric_instruction_count();
        let bytecode_hoisted_var_count = bytecode.hoist_plan().var_declaration_count();
        let bytecode_hoisted_function_count = bytecode.hoist_plan().function_declaration_count();
        Ok(Self {
            bytecode,
            usage: CompiledScriptUsage {
                source_len: source.len(),
                top_level_statement_count: parsed.usage.top_level_statement_count,
                max_expression_depth: parsed.usage.max_expression_depth,
                static_name_count: parsed.usage.static_name_count,
                static_string_count: parsed.usage.static_string_count,
                static_binding_count: parsed.usage.static_binding_count,
                static_property_access_count: parsed.usage.static_property_access_count,
                static_call_site_count: parsed.usage.static_call_site_count,
                resolved_static_binding_count: binding_layout.resolved_count(),
                unresolved_static_binding_count: binding_layout.unresolved_count(),
                global_binding_slot_count: binding_layout.global_slot_count(),
                local_binding_slot_count: binding_layout.local_slot_count(),
                upvalue_binding_slot_count: binding_layout.upvalue_slot_count(),
                bytecode_instruction_count,
                bytecode_binding_operand_count,
                bytecode_property_operand_count,
                bytecode_direct_native_call_count,
                bytecode_array_native_call_count,
                bytecode_numeric_instruction_count,
                bytecode_hoisted_var_count,
                bytecode_hoisted_function_count,
            },
            binding_layout,
        })
    }

    pub(crate) const fn binding_layout(&self) -> &BindingLayout {
        &self.binding_layout
    }

    pub(crate) const fn bytecode(&self) -> &BytecodeProgram {
        &self.bytecode
    }

    #[must_use]
    pub const fn usage(&self) -> CompiledScriptUsage {
        self.usage
    }

    pub(crate) fn ensure_within_limits(&self, limits: RuntimeLimits) -> Result<()> {
        check_source_len_value(self.usage.source_len, limits)?;
        if self.usage.top_level_statement_count > limits.max_statements {
            return Err(Error::limit(format!(
                "compiled script statement count {} exceeded {}",
                self.usage.top_level_statement_count, limits.max_statements
            )));
        }
        if self.usage.max_expression_depth > limits.max_expression_depth {
            return Err(Error::limit(format!(
                "compiled script expression nesting {} exceeded {}",
                self.usage.max_expression_depth, limits.max_expression_depth
            )));
        }
        Ok(())
    }
}

fn check_source_len(source: &str, limits: RuntimeLimits) -> Result<()> {
    check_source_len_value(source.len(), limits)
}

fn check_source_len_value(source_len: usize, limits: RuntimeLimits) -> Result<()> {
    if source_len > limits.max_source_len {
        return Err(Error::limit(format!(
            "source length {source_len} exceeded {}",
            limits.max_source_len
        )));
    }
    Ok(())
}
