mod usage;

pub use usage::CompiledScriptUsage;

use crate::{
    binding_metadata::BindingLayout,
    bytecode::BytecodeProgram,
    compiled_module::{ModuleExport, ModuleImport, ModuleImportName},
    compiler,
    error::{Error, Result},
    lexer,
    parser::{self, ModuleSyntax},
    runtime::limits::RuntimeLimits,
    source::SourceId,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScript {
    bytecode: BytecodeProgram,
    binding_layout: BindingLayout,
    usage: CompiledScriptUsage,
    source_id: SourceId,
    source_name: Option<String>,
    strict: bool,
}

#[derive(Clone, Copy)]
enum CompileMode {
    Script,
    Module,
    Eval {
        strict: bool,
        super_context: EvalSuperContext,
    },
}

#[derive(Clone, Copy)]
enum EvalSuperContext {
    None,
    Property,
    PropertyAndCall,
}

type CompiledModuleParts = (
    CompiledScript,
    Box<[String]>,
    Box<[ModuleImport]>,
    Box<[ModuleExport]>,
);

impl CompileMode {
    const SCRIPT: Self = Self::Script;

    const fn eval(strict: bool, allow_super_property: bool, allow_super_call: bool) -> Self {
        let super_context = match (allow_super_property, allow_super_call) {
            (_, true) => EvalSuperContext::PropertyAndCall,
            (true, false) => EvalSuperContext::Property,
            (false, false) => EvalSuperContext::None,
        };
        Self::Eval {
            strict,
            super_context,
        }
    }

    const fn strict(self) -> bool {
        match self {
            Self::Script => false,
            Self::Module => true,
            Self::Eval { strict, .. } => strict,
        }
    }

    const fn eval_super_context(self) -> Option<EvalSuperContext> {
        match self {
            Self::Script | Self::Module => None,
            Self::Eval { super_context, .. } => Some(super_context),
        }
    }
}

impl CompiledScript {
    pub(crate) fn compile(source: &str, limits: RuntimeLimits) -> Result<Self> {
        Self::compile_with_name_and_mode(None, source, limits, CompileMode::SCRIPT)
    }

    pub(crate) fn compile_named(
        source_name: &str,
        source: &str,
        limits: RuntimeLimits,
    ) -> Result<Self> {
        Self::compile_with_name_and_mode(Some(source_name), source, limits, CompileMode::SCRIPT)
    }

    pub(crate) fn compile_eval(
        source: &str,
        limits: RuntimeLimits,
        strict_mode: bool,
        allow_super_property: bool,
        allow_super_call: bool,
    ) -> Result<Self> {
        Self::compile_with_name_and_mode(
            None,
            source,
            limits,
            CompileMode::eval(strict_mode, allow_super_property, allow_super_call),
        )
    }

    pub(crate) fn compile_module_named(
        source_name: &str,
        source: &str,
        limits: RuntimeLimits,
    ) -> Result<CompiledModuleParts> {
        let (script, module) = Self::compile_with_name_and_mode_parts(
            Some(source_name),
            source,
            limits,
            CompileMode::Module,
        )?;
        let module = module.ok_or_else(|| Error::runtime("module parser did not return syntax"))?;
        let imported_exports = module
            .imports
            .iter()
            .map(|entry| {
                (
                    entry.local_name.clone(),
                    (entry.request.clone(), entry.import_name.clone()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let imports = module
            .imports
            .into_iter()
            .map(|entry| {
                ModuleImport::new(
                    entry.request,
                    match entry.import_name {
                        parser::ModuleImportName::Name(name) => ModuleImportName::Name(name),
                        parser::ModuleImportName::Namespace => ModuleImportName::Namespace,
                    },
                    entry.local_name,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let exports = module
            .exports
            .into_iter()
            .map(|entry| match entry {
                parser::ModuleExportEntry::Local {
                    export_name,
                    local_name,
                } => match imported_exports.get(&local_name) {
                    Some((request, parser::ModuleImportName::Name(import_name))) => {
                        ModuleExport::Indirect {
                            export_name,
                            import_name: import_name.clone(),
                            request: request.clone(),
                        }
                    }
                    Some((request, parser::ModuleImportName::Namespace)) => {
                        ModuleExport::Namespace {
                            export_name,
                            request: request.clone(),
                        }
                    }
                    None => ModuleExport::Local {
                        export_name,
                        local_name,
                    },
                },
                parser::ModuleExportEntry::Indirect {
                    export_name,
                    import_name,
                    request,
                } => ModuleExport::Indirect {
                    export_name,
                    import_name,
                    request,
                },
                parser::ModuleExportEntry::Namespace {
                    export_name,
                    request,
                } => ModuleExport::Namespace {
                    export_name,
                    request,
                },
                parser::ModuleExportEntry::Star { request } => ModuleExport::Star { request },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok((script, module.requests.into_boxed_slice(), imports, exports))
    }

    fn compile_with_name_and_mode(
        source_name: Option<&str>,
        source: &str,
        limits: RuntimeLimits,
        mode: CompileMode,
    ) -> Result<Self> {
        Self::compile_with_name_and_mode_parts(source_name, source, limits, mode)
            .map(|(script, _)| script)
    }

    fn compile_with_name_and_mode_parts(
        source_name: Option<&str>,
        source: &str,
        limits: RuntimeLimits,
        mode: CompileMode,
    ) -> Result<(Self, Option<ModuleSyntax>)> {
        check_source_len(source, &limits)?;
        check_source_name_len(source_name, &limits)?;
        let source_id = SourceId::for_optional_name(source_name, source);
        let tokens =
            lexer::lex(source, source_id).map_err(|error| error.with_source(source_id, source))?;
        let parsed = if matches!(mode, CompileMode::Module) {
            parser::parse_module_with_usage(tokens, limits)
        } else if let Some(super_context) = mode.eval_super_context() {
            let allow_super_property = matches!(
                super_context,
                EvalSuperContext::Property | EvalSuperContext::PropertyAndCall
            );
            let allow_super_call = matches!(super_context, EvalSuperContext::PropertyAndCall);
            parser::parse_eval_with_usage_in_context(
                tokens,
                limits,
                mode.strict(),
                allow_super_property,
                allow_super_call,
            )
        } else if mode.strict() {
            parser::parse_with_usage_in_mode(tokens, limits, true)
        } else {
            parser::parse_with_usage(tokens, limits)
        }
        .map_err(|error| error.with_source(source_id, source))?;
        let module = parsed.module;
        let program = parsed.program;
        let binding_layout = match mode {
            CompileMode::Eval { .. } => BindingLayout::build_eval(
                &program,
                parsed.usage.static_binding_count,
                parsed.usage.static_function_count,
                parsed.strict,
            )?,
            CompileMode::Module => BindingLayout::build_module(
                &program,
                parsed.usage.static_binding_count,
                parsed.usage.static_function_count,
            )?,
            CompileMode::Script => BindingLayout::build(
                &program,
                parsed.usage.static_binding_count,
                parsed.usage.static_function_count,
            )?,
        };
        let bytecode = if let Some(module) = module.as_ref() {
            let import_local_names = module
                .imports
                .iter()
                .map(|entry| entry.local_name.as_str())
                .collect();
            compiler::compile_module_program(&program, &binding_layout, &import_local_names)?
        } else {
            compiler::compile_program(&program, &binding_layout)?
        };
        let bytecode_instruction_count = bytecode.instruction_count();
        let bytecode_binding_operand_count = bytecode.binding_operand_count();
        let bytecode_property_operand_count = bytecode.property_operand_count();
        let bytecode_direct_native_call_count = bytecode.direct_native_call_count();
        let bytecode_array_native_call_count = bytecode.array_native_call_count();
        let bytecode_numeric_instruction_count = bytecode.numeric_instruction_count();
        let bytecode_hoisted_var_count = bytecode.hoist_plan().var_declaration_count();
        let bytecode_hoisted_function_count = bytecode.hoist_plan().function_declaration_count();
        let script = Self {
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
            source_id,
            source_name: source_name.map(str::to_owned),
            strict: parsed.strict,
        };
        Ok((script, module))
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

    /// Returns the stable diagnostic identity of the compiled source.
    #[must_use]
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// Returns the embedder-provided source name, when compilation was named.
    #[must_use]
    pub fn source_name(&self) -> Option<&str> {
        self.source_name.as_deref()
    }

    pub(crate) const fn strict(&self) -> bool {
        self.strict
    }

    pub(crate) fn ensure_within_limits(&self, limits: &RuntimeLimits) -> Result<()> {
        check_source_len_value(self.usage.source_len, limits)?;
        check_source_name_len(self.source_name(), limits)?;
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

fn check_source_len(source: &str, limits: &RuntimeLimits) -> Result<()> {
    check_source_len_value(source.len(), limits)
}

fn check_source_len_value(source_len: usize, limits: &RuntimeLimits) -> Result<()> {
    if source_len > limits.max_source_len {
        return Err(Error::limit(format!(
            "source length {source_len} exceeded {}",
            limits.max_source_len
        )));
    }
    Ok(())
}

fn check_source_name_len(source_name: Option<&str>, limits: &RuntimeLimits) -> Result<()> {
    let Some(source_name) = source_name else {
        return Ok(());
    };
    if source_name.len() > limits.max_string_len {
        return Err(Error::limit(format!(
            "source name length {} exceeded {}",
            source_name.len(),
            limits.max_string_len
        )));
    }
    Ok(())
}
