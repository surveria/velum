use crate::{
    ast::Program,
    binding_layout::BindingLayout,
    error::{Error, Result},
    lexer, parser,
    runtime_limits::RuntimeLimits,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScript {
    program: Program,
    binding_layout: BindingLayout,
    usage: CompiledScriptUsage,
}

impl CompiledScript {
    pub(crate) fn compile(source: &str, limits: RuntimeLimits) -> Result<Self> {
        check_source_len(source, limits)?;
        let tokens = lexer::lex(source)?;
        let parsed = parser::parse_with_usage(tokens, limits)?;
        let binding_layout = BindingLayout::build(
            &parsed.program,
            parsed.usage.static_binding_count,
            parsed.usage.static_function_count,
        )?;
        Ok(Self {
            program: parsed.program,
            usage: CompiledScriptUsage {
                source_len: source.len(),
                top_level_statement_count: parsed.usage.top_level_statement_count,
                max_expression_depth: parsed.usage.max_expression_depth,
                static_name_count: parsed.usage.static_name_count,
                static_binding_count: parsed.usage.static_binding_count,
                resolved_static_binding_count: binding_layout.resolved_count(),
                unresolved_static_binding_count: binding_layout.unresolved_count(),
                global_binding_slot_count: binding_layout.global_slot_count(),
                local_binding_slot_count: binding_layout.local_slot_count(),
                upvalue_binding_slot_count: binding_layout.upvalue_slot_count(),
            },
            binding_layout,
        })
    }

    pub(crate) const fn binding_layout(&self) -> &BindingLayout {
        &self.binding_layout
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

    pub(crate) const fn program(&self) -> &Program {
        &self.program
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CompiledScriptUsage {
    source_len: usize,
    top_level_statement_count: usize,
    max_expression_depth: usize,
    static_name_count: usize,
    static_binding_count: usize,
    resolved_static_binding_count: usize,
    unresolved_static_binding_count: usize,
    global_binding_slot_count: usize,
    local_binding_slot_count: usize,
    upvalue_binding_slot_count: usize,
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
    pub const fn static_binding_count(self) -> usize {
        self.static_binding_count
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
