/// The reason an embedding requested termination of matching.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InterruptReason {
    Cancelled,
    HostStepLimit,
}

/// Embedding hook used to share execution accounting and cancellation.
pub trait ExecutionControl {
    /// Charges host-owned execution work.
    ///
    /// # Errors
    ///
    /// Returns an interruption reason when the embedding cancels execution or
    /// exhausts its shared step budget.
    fn charge_steps(&mut self, steps: usize) -> Result<(), InterruptReason>;
}

/// A standalone control that never interrupts execution.
#[derive(Debug, Default)]
pub struct NoopExecutionControl;

impl ExecutionControl for NoopExecutionControl {
    fn charge_steps(&mut self, _steps: usize) -> Result<(), InterruptReason> {
        Ok(())
    }
}
