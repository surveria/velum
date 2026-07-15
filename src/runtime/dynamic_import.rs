use crate::{
    compiled_module::DynamicModuleRequest,
    error::{Error, JavaScriptErrorMetadata, Result},
    runtime::{Context, control::runtime_exception_value, promise::PromiseId},
    syntax::ImportPhase,
    value::{ErrorName, Value},
};

#[derive(Debug)]
pub(in crate::runtime) struct DynamicImportJob {
    promise: PromiseId,
    referrer: String,
    request: DynamicModuleRequest,
}

impl DynamicImportJob {
    pub(in crate::runtime) const fn promise(&self) -> PromiseId {
        self.promise
    }
}

impl Context {
    pub(in crate::runtime) fn enqueue_dynamic_import(
        &mut self,
        phase: ImportPhase,
        specifier: String,
        options: &Value,
    ) -> Result<Value> {
        let attributes = self.dynamic_import_attributes(options)?;
        let referrer = self.active_script_or_module_name().unwrap_or_default();
        let request = DynamicModuleRequest::new(specifier, phase, attributes);
        let (promise, object) = self.create_pending_promise()?;
        self.enqueue_promise_job(crate::runtime::promise::PromiseJob::DynamicImport(
            DynamicImportJob {
                promise,
                referrer,
                request,
            },
        ))?;
        Ok(object)
    }

    fn dynamic_import_attributes(&mut self, options: &Value) -> Result<Box<[(String, String)]>> {
        if matches!(options, Value::Undefined) {
            return Ok(Box::new([]));
        }
        if self.semantic_object_ref(options)?.is_none() {
            return Err(Error::type_error(
                "dynamic import options must be an object",
            ));
        }
        let attributes = self.get_named(options, "with")?;
        if matches!(attributes, Value::Undefined) {
            return Ok(Box::new([]));
        }
        if self.semantic_object_ref(&attributes)?.is_none() {
            return Err(Error::type_error(
                "dynamic import 'with' option must be an object",
            ));
        }
        let entries = self.semantic_enumerable_own_string_entries(&attributes)?;
        let mut normalized = Vec::with_capacity(entries.len());
        for (key, value) in entries {
            let Some(value) = value.string_text() else {
                return Err(Error::type_error(
                    "dynamic import attribute values must be strings",
                ));
            };
            normalized.push((key, value.to_owned()));
        }
        normalized.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(normalized.into_boxed_slice())
    }

    pub(in crate::runtime) fn run_dynamic_import_job(
        &mut self,
        job: DynamicImportJob,
    ) -> Result<()> {
        let DynamicImportJob {
            promise,
            referrer,
            request,
        } = job;
        if request.phase() == ImportPhase::Evaluation {
            let result = self.begin_dynamic_module_namespace(&referrer, &request);
            return match result {
                Ok((evaluation, namespace)) => self.add_promise_reaction(
                    evaluation,
                    crate::runtime::promise::PromiseReaction::dynamic_import_module(
                        promise, namespace,
                    ),
                ),
                Err(error) => {
                    let reason = self.dynamic_import_error_value(&error)?;
                    self.reject_promise(promise, reason)
                }
            };
        }
        let result = self.load_dynamic_module_namespace(&referrer, &request);
        match result {
            Ok(namespace) => self.resolve_promise(promise, namespace),
            Err(error) => {
                let reason = self.dynamic_import_error_value(&error)?;
                self.reject_promise(promise, reason)
            }
        }
    }

    pub(in crate::runtime) fn dynamic_import_error_value(
        &mut self,
        error: &Error,
    ) -> Result<Value> {
        if let Some(reason) = runtime_exception_value(self, error)? {
            return Ok(reason);
        }
        let name = match error {
            Error::Lex { .. } | Error::Parse { .. } => ErrorName::SyntaxError,
            Error::ResourceLimit { .. } => ErrorName::RangeError,
            Error::Runtime { .. } => ErrorName::TypeError,
            Error::JavaScript { .. } | Error::JavaScriptError { .. } => {
                return Err(Error::runtime(
                    "dynamic import JavaScript error lost its exception value",
                ));
            }
        };
        self.create_error_object(JavaScriptErrorMetadata::new(name, error.to_string()), true)
    }
}
