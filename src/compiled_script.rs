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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompiledScriptUsage {
    source_len: usize,
    top_level_statement_count: usize,
    max_expression_depth: usize,
    static_name_count: usize,
    static_string_count: usize,
    static_binding_count: usize,
    static_property_access_count: usize,
    static_call_site_count: usize,
    resolved_static_binding_count: usize,
    unresolved_static_binding_count: usize,
    global_binding_slot_count: usize,
    local_binding_slot_count: usize,
    upvalue_binding_slot_count: usize,
    bytecode_instruction_count: usize,
    bytecode_binding_operand_count: usize,
    bytecode_property_operand_count: usize,
    bytecode_direct_native_call_count: usize,
    bytecode_array_native_call_count: usize,
    bytecode_numeric_instruction_count: usize,
    bytecode_hoisted_var_count: usize,
    bytecode_hoisted_function_count: usize,
}

impl CompiledScriptUsage {
    #[must_use]
    pub const fn source_len(self) -> usize {
        self.source_len
    }

    #[must_use]
    pub const fn top_level_statement_count(self) -> usize {
        self.top_level_statement_count
    }

    #[must_use]
    pub const fn max_expression_depth(self) -> usize {
        self.max_expression_depth
    }

    #[must_use]
    pub const fn static_name_count(self) -> usize {
        self.static_name_count
    }

    #[must_use]
    pub const fn static_string_count(self) -> usize {
        self.static_string_count
    }

    #[must_use]
    pub const fn static_binding_count(self) -> usize {
        self.static_binding_count
    }

    #[must_use]
    pub const fn static_property_access_count(self) -> usize {
        self.static_property_access_count
    }

    #[must_use]
    pub const fn static_call_site_count(self) -> usize {
        self.static_call_site_count
    }

    #[must_use]
    pub const fn resolved_static_binding_count(self) -> usize {
        self.resolved_static_binding_count
    }

    #[must_use]
    pub const fn unresolved_static_binding_count(self) -> usize {
        self.unresolved_static_binding_count
    }

    #[must_use]
    pub const fn global_binding_slot_count(self) -> usize {
        self.global_binding_slot_count
    }

    #[must_use]
    pub const fn local_binding_slot_count(self) -> usize {
        self.local_binding_slot_count
    }

    #[must_use]
    pub const fn upvalue_binding_slot_count(self) -> usize {
        self.upvalue_binding_slot_count
    }

    #[must_use]
    pub const fn bytecode_instruction_count(self) -> usize {
        self.bytecode_instruction_count
    }

    #[must_use]
    pub const fn bytecode_binding_operand_count(self) -> usize {
        self.bytecode_binding_operand_count
    }

    #[must_use]
    pub const fn bytecode_property_operand_count(self) -> usize {
        self.bytecode_property_operand_count
    }

    #[must_use]
    pub const fn bytecode_direct_native_call_count(self) -> usize {
        self.bytecode_direct_native_call_count
    }

    #[must_use]
    pub const fn bytecode_array_native_call_count(self) -> usize {
        self.bytecode_array_native_call_count
    }

    #[must_use]
    pub const fn bytecode_numeric_instruction_count(self) -> usize {
        self.bytecode_numeric_instruction_count
    }

    #[must_use]
    pub const fn bytecode_hoisted_var_count(self) -> usize {
        self.bytecode_hoisted_var_count
    }

    #[must_use]
    pub const fn bytecode_hoisted_function_count(self) -> usize {
        self.bytecode_hoisted_function_count
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
