use crate::{
    compiled_script::{CompiledScript, CompiledScriptUsage},
    error::Result,
    parser::{ModuleExportEntry, ModuleImportName as ParsedImportName, ModuleSyntax},
    runtime::limits::RuntimeLimits,
    source::SourceId,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ModuleImportName {
    Name(String),
    Namespace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleImport {
    request: String,
    import_name: ModuleImportName,
    local_name: String,
}

impl ModuleImport {
    #[must_use]
    pub const fn request(&self) -> &str {
        self.request.as_str()
    }

    #[must_use]
    pub const fn import_name(&self) -> &ModuleImportName {
        &self.import_name
    }

    #[must_use]
    pub const fn local_name(&self) -> &str {
        self.local_name.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ModuleExport {
    Local {
        export_name: String,
        local_name: String,
    },
    Indirect {
        export_name: String,
        import_name: String,
        request: String,
    },
    Namespace {
        export_name: String,
        request: String,
    },
    Star {
        request: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModule {
    script: CompiledScript,
    requests: Box<[String]>,
    imports: Box<[ModuleImport]>,
    exports: Box<[ModuleExport]>,
}

impl CompiledModule {
    pub(crate) fn compile_named(
        source_name: &str,
        source: &str,
        limits: RuntimeLimits,
    ) -> Result<Self> {
        let (script, syntax) = CompiledScript::compile_module_named(source_name, source, limits)?;
        Ok(Self::from_syntax(script, syntax))
    }

    fn from_syntax(script: CompiledScript, syntax: ModuleSyntax) -> Self {
        let imports = syntax
            .imports
            .into_iter()
            .map(|entry| ModuleImport {
                request: entry.request,
                import_name: match entry.import_name {
                    ParsedImportName::Name(name) => ModuleImportName::Name(name),
                    ParsedImportName::Namespace => ModuleImportName::Namespace,
                },
                local_name: entry.local_name,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let exports = syntax
            .exports
            .into_iter()
            .map(|entry| match entry {
                ModuleExportEntry::Local {
                    export_name,
                    local_name,
                } => ModuleExport::Local {
                    export_name,
                    local_name,
                },
                ModuleExportEntry::Indirect {
                    export_name,
                    import_name,
                    request,
                } => ModuleExport::Indirect {
                    export_name,
                    import_name,
                    request,
                },
                ModuleExportEntry::Namespace {
                    export_name,
                    request,
                } => ModuleExport::Namespace {
                    export_name,
                    request,
                },
                ModuleExportEntry::Star { request } => ModuleExport::Star { request },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            script,
            requests: syntax.requests.into_boxed_slice(),
            imports,
            exports,
        }
    }

    #[must_use]
    pub const fn requests(&self) -> &[String] {
        &self.requests
    }

    #[must_use]
    pub const fn imports(&self) -> &[ModuleImport] {
        &self.imports
    }

    #[must_use]
    pub const fn exports(&self) -> &[ModuleExport] {
        &self.exports
    }

    #[must_use]
    pub const fn usage(&self) -> CompiledScriptUsage {
        self.script.usage()
    }

    #[must_use]
    pub const fn source_id(&self) -> SourceId {
        self.script.source_id()
    }

    #[must_use]
    pub fn source_name(&self) -> Option<&str> {
        self.script.source_name()
    }
}
