mod usage;

pub use usage::CompiledScriptUsage;

use crate::{
    binding_metadata::BindingLayout,
    bytecode::BytecodeProgram,
    compiled_module::{ModuleExport, ModuleImport, ModuleImportName, ModuleRequest},
    compiler,
    error::{Error, Result},
    lexer,
    parser::{self, ModuleSyntax},
    runtime::limits::RuntimeLimits,
    source::SourceId,
    syntax::StaticName,
};
use std::collections::BTreeMap;
use std::rc::Rc;

pub use crate::parser::{EvalClassFieldContext, EvalSuperContext};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledScript {
    bytecode: BytecodeProgram,
    binding_layout: BindingLayout,
    usage: CompiledScriptUsage,
    source_id: SourceId,
    source_name: Option<String>,
    strict: bool,
    top_level_await: bool,
}

#[derive(Clone)]
enum CompileMode {
    Script,
    Module,
    Eval {
        strict: bool,
        super_context: EvalSuperContext,
        class_field_context: EvalClassFieldContext,
        allow_new_target: bool,
        private_names: Rc<[StaticName]>,
    },
}

pub struct EvalCompileContext {
    strict: bool,
    super_context: EvalSuperContext,
    class_field_context: EvalClassFieldContext,
    allow_new_target: bool,
    private_names: Rc<[StaticName]>,
}

type CompiledModuleParts = (
    CompiledScript,
    Box<[String]>,
    Box<[ModuleRequest]>,
    Box<[ModuleImport]>,
    Box<[ModuleExport]>,
);

fn compile_module_requests(module: &ModuleSyntax) -> (Box<[String]>, Box<[ModuleRequest]>) {
    let module_requests = module
        .requests
        .iter()
        .map(|request| {
            ModuleRequest::new(
                request.specifier.clone(),
                request.phase,
                request.attributes.clone(),
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut requests = Vec::new();
    for request in &module.requests {
        if !requests.iter().any(|known| known == &request.specifier) {
            requests.push(request.specifier.clone());
        }
    }
    (requests.into_boxed_slice(), module_requests)
}

impl CompileMode {
    const SCRIPT: Self = Self::Script;

    fn eval(context: EvalCompileContext) -> Self {
        Self::Eval {
            strict: context.strict,
            super_context: context.super_context,
            class_field_context: context.class_field_context,
            allow_new_target: context.allow_new_target,
            private_names: context.private_names,
        }
    }

    const fn strict(&self) -> bool {
        match self {
            Self::Script => false,
            Self::Module => true,
            Self::Eval { strict, .. } => *strict,
        }
    }

    fn eval_context(&self) -> Option<parser::EvalParseContext<'_>> {
        match self {
            Self::Script | Self::Module => None,
            Self::Eval {
                strict,
                super_context,
                class_field_context,
                allow_new_target,
                private_names,
            } => Some(parser::EvalParseContext::new(
                *strict,
                *super_context,
                *class_field_context,
                *allow_new_target,
                private_names,
            )),
        }
    }
}

impl EvalCompileContext {
    pub(crate) const fn new(
        strict: bool,
        super_context: EvalSuperContext,
        class_field_context: EvalClassFieldContext,
        allow_new_target: bool,
        private_names: Rc<[StaticName]>,
    ) -> Self {
        Self {
            strict,
            super_context,
            class_field_context,
            allow_new_target,
            private_names,
        }
    }
}

impl CompiledScript {
    pub(crate) fn compile(source: &str, limits: RuntimeLimits) -> Result<Self> {
        Self::compile_with_name_and_mode(None, source, limits, &CompileMode::SCRIPT)
    }

    pub(crate) fn compile_named(
        source_name: &str,
        source: &str,
        limits: RuntimeLimits,
    ) -> Result<Self> {
        Self::compile_with_name_and_mode(Some(source_name), source, limits, &CompileMode::SCRIPT)
    }

    pub(crate) fn compile_eval(
        source: &str,
        limits: RuntimeLimits,
        context: EvalCompileContext,
    ) -> Result<Self> {
        let mode = CompileMode::eval(context);
        Self::compile_with_name_and_mode(None, source, limits, &mode)
    }

    pub(crate) fn compile_eval_utf16(
        source: &[u16],
        limits: RuntimeLimits,
        context: EvalCompileContext,
    ) -> Result<Self> {
        let mode = CompileMode::eval(context);
        let source_id = SourceId::for_optional_name_utf16(None, source);
        Self::compile_with_source_text_parts(
            None,
            lexer::SourceText::from_utf16(source),
            source_id,
            limits,
            &mode,
        )
        .map(|(script, _)| script)
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
            &CompileMode::Module,
        )?;
        let module = module.ok_or_else(|| Error::runtime("module parser did not return syntax"))?;
        let (requests, module_requests) = compile_module_requests(&module);
        let imported_exports = module
            .imports
            .iter()
            .map(|entry| {
                (
                    entry.local_name.clone(),
                    (
                        ModuleRequest::new(
                            entry.request.specifier.clone(),
                            entry.request.phase,
                            entry.request.attributes.clone(),
                        ),
                        entry.import_name.clone(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let imports = module
            .imports
            .into_iter()
            .map(|entry| {
                ModuleImport::new(
                    ModuleRequest::new(
                        entry.request.specifier,
                        entry.request.phase,
                        entry.request.attributes,
                    ),
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
                            request: request.specifier().to_owned(),
                        }
                    }
                    Some((request, parser::ModuleImportName::Namespace)) => {
                        if request.phase() == crate::syntax::ImportPhase::Defer {
                            ModuleExport::DeferredNamespace {
                                export_name,
                                request: request.specifier().to_owned(),
                            }
                        } else {
                            ModuleExport::Namespace {
                                export_name,
                                request: request.specifier().to_owned(),
                            }
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
        Ok((script, requests, module_requests, imports, exports))
    }

    fn compile_with_name_and_mode(
        source_name: Option<&str>,
        source: &str,
        limits: RuntimeLimits,
        mode: &CompileMode,
    ) -> Result<Self> {
        Self::compile_with_name_and_mode_parts(source_name, source, limits, mode)
            .map(|(script, _)| script)
    }

    fn compile_with_name_and_mode_parts(
        source_name: Option<&str>,
        source: &str,
        limits: RuntimeLimits,
        mode: &CompileMode,
    ) -> Result<(Self, Option<ModuleSyntax>)> {
        let source_id = SourceId::for_optional_name(source_name, source);
        Self::compile_with_source_text_parts(
            source_name,
            lexer::SourceText::from_utf8(source),
            source_id,
            limits,
            mode,
        )
    }

    fn compile_with_source_text_parts(
        source_name: Option<&str>,
        source: lexer::SourceText,
        source_id: SourceId,
        limits: RuntimeLimits,
        mode: &CompileMode,
    ) -> Result<(Self, Option<ModuleSyntax>)> {
        check_source_len_value(source.source_len(), &limits)?;
        check_source_name_len(source_name, &limits)?;
        let source_len = source.source_len();
        let diagnostic_source = source.rendered().to_owned();
        let allow_html_comments = !matches!(mode, CompileMode::Module);
        let tokens = lexer::TokenStream::from_source_text(source, source_id, allow_html_comments);
        let parsed = if matches!(mode, CompileMode::Module) {
            parser::parse_module_with_usage(tokens, limits)
        } else if let Some(context) = mode.eval_context() {
            parser::parse_eval_with_usage_in_context(tokens, limits, context)
        } else if mode.strict() {
            parser::parse_with_usage_in_mode(tokens, limits, true)
        } else {
            parser::parse_with_usage(tokens, limits)
        }
        .map_err(|error| error.with_source(source_id, &diagnostic_source))?;
        let module = parsed.module;
        let program = parsed.program;
        let mut binding_layout = match mode {
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
        binding_layout.set_source_text(Rc::from(diagnostic_source.into_boxed_str()));
        let bytecode = compiler::compile_program(&program, &binding_layout)?;
        binding_layout.clear_source_text();
        let bytecode_metrics = bytecode.metrics();
        let bytecode_hoisted_var_count = bytecode.hoist_plan().var_declaration_count();
        let bytecode_hoisted_function_count = bytecode.hoist_plan().function_declaration_count();
        let script = Self {
            bytecode,
            usage: CompiledScriptUsage {
                source_len,
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
                bytecode_instruction_count: bytecode_metrics.instruction_count(),
                bytecode_binding_operand_count: bytecode_metrics.binding_operand_count(),
                bytecode_property_operand_count: bytecode_metrics.property_operand_count(),
                bytecode_direct_native_call_count: bytecode_metrics.direct_native_call_count(),
                bytecode_array_native_call_count: bytecode_metrics.array_native_call_count(),
                bytecode_numeric_instruction_count: bytecode_metrics.numeric_instruction_count(),
                bytecode_linear_peephole_candidate_count: bytecode_metrics
                    .linear_peephole_candidate_count(),
                bytecode_numeric_array_reduction_role_count: bytecode_metrics
                    .numeric_array_reduction_role_count(),
                bytecode_hoisted_var_count,
                bytecode_hoisted_function_count,
            },
            binding_layout,
            source_id,
            source_name: source_name.map(str::to_owned),
            strict: parsed.strict,
            top_level_await: parsed.usage.top_level_await,
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

    pub(crate) const fn has_top_level_await(&self) -> bool {
        self.top_level_await
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
