#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompiledScriptUsage {
    pub(super) source_len: usize,
    pub(super) top_level_statement_count: usize,
    pub(super) max_expression_depth: usize,
    pub(super) static_name_count: usize,
    pub(super) static_string_count: usize,
    pub(super) static_binding_count: usize,
    pub(super) static_property_access_count: usize,
    pub(super) static_call_site_count: usize,
    pub(super) resolved_static_binding_count: usize,
    pub(super) unresolved_static_binding_count: usize,
    pub(super) global_binding_slot_count: usize,
    pub(super) local_binding_slot_count: usize,
    pub(super) upvalue_binding_slot_count: usize,
    pub(super) bytecode_instruction_count: usize,
    pub(super) bytecode_binding_operand_count: usize,
    pub(super) bytecode_property_operand_count: usize,
    pub(super) bytecode_direct_native_call_count: usize,
    pub(super) bytecode_array_native_call_count: usize,
    pub(super) bytecode_numeric_instruction_count: usize,
    pub(super) bytecode_linear_peephole_candidate_count: usize,
    pub(super) bytecode_numeric_array_reduction_role_count: usize,
    pub(super) bytecode_hoisted_var_count: usize,
    pub(super) bytecode_hoisted_function_count: usize,
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
    pub const fn bytecode_linear_peephole_candidate_count(self) -> usize {
        self.bytecode_linear_peephole_candidate_count
    }

    #[must_use]
    pub const fn bytecode_numeric_array_reduction_role_count(self) -> usize {
        self.bytecode_numeric_array_reduction_role_count
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
