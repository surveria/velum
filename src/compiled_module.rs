use crate::{
    compiled_script::{CompiledScript, CompiledScriptUsage},
    error::Result,
    runtime::limits::RuntimeLimits,
    source::SourceId,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ModuleSource {
    specifier: String,
    source: String,
}

impl ModuleSource {
    #[must_use]
    pub fn new(specifier: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            specifier: specifier.into(),
            source: source.into(),
        }
    }

    #[must_use]
    pub const fn specifier(&self) -> &str {
        self.specifier.as_str()
    }

    #[must_use]
    pub const fn source(&self) -> &str {
        self.source.as_str()
    }
}

pub trait ModuleLoader {
    /// Resolves and loads one requested module source. The returned specifier
    /// is the canonical identity used for graph deduplication and cycles.
    ///
    /// # Errors
    /// Returns an embedder or policy error when resolution or loading fails.
    fn load(&mut self, referrer: &str, request: &str) -> Result<ModuleSource>;
}

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
    pub(crate) const fn new(
        request: String,
        import_name: ModuleImportName,
        local_name: String,
    ) -> Self {
        Self {
            request,
            import_name,
            local_name,
        }
    }

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
        let (script, requests, imports, exports) =
            CompiledScript::compile_module_named(source_name, source, limits)?;
        Ok(Self {
            script,
            requests,
            imports,
            exports,
        })
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

    pub(crate) const fn script(&self) -> &CompiledScript {
        &self.script
    }
}
