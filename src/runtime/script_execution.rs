use crate::{
    compiled_module::CompiledModule,
    compiled_script::CompiledScript,
    error::{Error, Result},
    runtime::{
        Context,
        binding::static_bindings::StaticBindingCacheHandle,
        bytecode::BytecodeOutcome,
        control::{Completion, runtime_exception_value},
        property::static_names::StaticNameAtomCacheHandle,
    },
    value::Value,
};

#[derive(Debug, Clone, Copy)]
enum ScriptExecutionMode {
    Script,
    SloppyEval,
    StrictEval,
}

impl Context {
    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits
    /// fail. An uncaught JavaScript value is returned as
    /// [`Error::JavaScript`](crate::Error::JavaScript).
    ///
    /// The returned raw value is not a durable root. Use `eval_owned` for a
    /// portable primitive or `eval_retained` across later Context calls.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        let script = self.compile(source)?;
        self.eval_compiled(&script)
    }

    /// Evaluates source with a stable embedder-provided diagnostic and module-referrer name.
    ///
    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits fail.
    pub fn eval_named(&mut self, source_name: &str, source: &str) -> Result<Value> {
        let script = self.compile_named(source_name, source)?;
        self.eval_compiled(&script)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits.clone())
    }

    /// Compiles source with a stable embedder-provided diagnostic name.
    ///
    /// # Errors
    /// Fails when the source name exceeds configured string limits, or when
    /// lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile_named(source_name, source, self.limits.clone())
    }

    /// Compiles an ECMAScript module with a stable embedder-provided specifier.
    ///
    /// # Errors
    /// Fails when module lexing, parsing, static validation, or configured
    /// compile-time resource limits fail.
    pub fn compile_module_named(&self, source_name: &str, source: &str) -> Result<CompiledModule> {
        CompiledModule::compile_named(source_name, source, self.limits.clone())
    }

    /// # Errors
    /// Fails when the compiled script exceeds this context's limits or evaluation fails.
    pub fn eval_compiled(&mut self, script: &CompiledScript) -> Result<Value> {
        let outcome = self.eval_compiled_outcome(script, ScriptExecutionMode::Script)?;
        let span = outcome.span();
        let completion = outcome.completion();
        let Completion::Throw(value) = completion else {
            let result = completion.into_result();
            return if let Some(span) = span {
                result.map_err(|error| error.with_runtime_span(span))
            } else {
                result
            };
        };
        let metadata = if let Value::Object(id) = &value {
            self.objects.error_metadata(*id)?.cloned()
        } else {
            None
        };
        Err(Error::javascript_with_metadata(
            self.identity.clone(),
            value,
            metadata,
            span,
        ))
    }

    pub(crate) fn eval_compiled_eval_completion(
        &mut self,
        script: &CompiledScript,
        strict: bool,
    ) -> Result<Completion> {
        let mode = if strict {
            ScriptExecutionMode::StrictEval
        } else {
            ScriptExecutionMode::SloppyEval
        };
        self.eval_compiled_outcome(script, mode)
            .map(BytecodeOutcome::completion)
    }

    fn eval_compiled_outcome(
        &mut self,
        script: &CompiledScript,
        mode: ScriptExecutionMode,
    ) -> Result<BytecodeOutcome> {
        script.ensure_within_limits(&self.limits)?;
        let static_name_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.with_static_name_caches(
            static_name_cache,
            binding_cache,
            script.binding_layout().clone(),
            |context| context.eval_compiled_with_mode(script, mode),
        )
    }

    fn eval_compiled_with_mode(
        &mut self,
        script: &CompiledScript,
        mode: ScriptExecutionMode,
    ) -> Result<BytecodeOutcome> {
        let plan = script.bytecode().hoist_plan();
        match mode {
            ScriptExecutionMode::Script => {
                if let Some(outcome) =
                    self.hoist_outcome(|context| context.hoist_bytecode_declarations(plan))?
                {
                    return Ok(outcome);
                }
                self.eval_compiled_program(script)
            }
            ScriptExecutionMode::SloppyEval => {
                if let Some(outcome) =
                    self.hoist_outcome(|context| context.hoist_bytecode_var_declarations(plan))?
                {
                    return Ok(outcome);
                }
                self.eval_compiled_in_lexical_scope(script, |context| {
                    context.hoist_bytecode_lexical_declarations(plan)
                })
            }
            ScriptExecutionMode::StrictEval => self
                .eval_compiled_in_lexical_scope(script, |context| {
                    context.hoist_bytecode_declarations(plan)
                }),
        }
    }

    fn eval_compiled_in_lexical_scope(
        &mut self,
        script: &CompiledScript,
        hoist: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<BytecodeOutcome> {
        self.push_lexical_scope()?;
        let outcome = match self.hoist_outcome(hoist) {
            Ok(Some(outcome)) => Ok(outcome),
            Ok(None) => self.eval_compiled_program(script),
            Err(error) => Err(error),
        };
        let pop_result = self.pop_lexical_scope();
        pop_result?;
        outcome
    }

    fn hoist_outcome(
        &mut self,
        hoist: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<Option<BytecodeOutcome>> {
        if let Err(error) = hoist(self) {
            let Some(value) = runtime_exception_value(self, &error)? else {
                return Err(error);
            };
            return Ok(Some(BytecodeOutcome::Completed {
                completion: Completion::Throw(value),
                span: None,
            }));
        }
        Ok(None)
    }

    fn eval_compiled_program(&mut self, script: &CompiledScript) -> Result<BytecodeOutcome> {
        let previous_source = script
            .source_name()
            .map(|name| self.active_module_name.replace(name.to_owned()));
        let result = self
            .eval_bytecode_program(script.bytecode())
            .and_then(|outcome| {
                if outcome.is_normal() {
                    self.drain_promise_jobs()?;
                }
                Ok(outcome)
            });
        if let Some(previous_source) = previous_source {
            self.active_module_name = previous_source;
        }
        result
    }
}
