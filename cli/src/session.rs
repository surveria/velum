use velum::{Context, Runtime, Value};

const PROMISE_JOB_ERROR_PREFIX: &str = "Promise job execution failed";

/// The complete observable result of one shell submission.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Submission {
    output: Vec<String>,
    value: Option<String>,
    errors: Vec<String>,
}

impl Submission {
    /// Returns lines emitted through the engine's `print(...)` host function.
    #[must_use]
    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Returns the display form of the script completion value.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// Returns evaluation and Promise-job errors in observation order.
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Reports whether evaluation and Promise-job draining both succeeded.
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        self.errors.is_empty()
    }
}

/// One persistent Velum execution context used by an interactive shell.
#[derive(Debug)]
pub struct ShellSession {
    context: Context,
}

impl ShellSession {
    /// Creates an empty shell session with the engine's default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            context: Runtime::new().context(),
        }
    }

    /// Evaluates one complete submission and drains ready Promise jobs.
    ///
    /// Output is captured even when evaluation throws. The runtime-step budget
    /// is restarted for each submission while global bindings and heap state
    /// remain owned by the same context.
    #[must_use]
    pub fn submit(&mut self, source_name: &str, source: &str) -> Submission {
        self.context.begin_runtime_step_budget();
        let evaluation = self.context.eval_named(source_name, source);
        let jobs = self.context.run_jobs();
        let output = self.context.take_output();
        let mut errors = Vec::with_capacity(2);

        let value = match evaluation {
            Ok(Value::Undefined) => None,
            Ok(value) => Some(value.to_string()),
            Err(error) => {
                errors.push(error.to_string());
                None
            }
        };

        if let Err(error) = jobs {
            errors.push(format!("{PROMISE_JOB_ERROR_PREFIX}: {error}"));
        }

        Submission {
            output,
            value,
            errors,
        }
    }

    /// Replaces all JavaScript state with a fresh context.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Runs an explicit full garbage collection for the current VM.
    ///
    /// # Errors
    ///
    /// Returns an error when VM reachability or storage-accounting invariants
    /// cannot be reconciled.
    pub fn collect_garbage(&mut self) -> velum::Result<()> {
        self.context.collect_garbage().map(|_| ())
    }
}

impl Default for ShellSession {
    fn default() -> Self {
        Self::new()
    }
}
