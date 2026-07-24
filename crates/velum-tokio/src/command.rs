use tokio::sync::{mpsc, oneshot};
use velum::Vm;

use crate::RuntimeError;

pub trait VmOperation: Send {
    fn execute(self: Box<Self>, vm: &mut Vm);
    fn reject(self: Box<Self>, error: RuntimeError);
}

pub struct TypedOperation<F, R> {
    callback: F,
    response: oneshot::Sender<Result<R, RuntimeError>>,
}

impl<F, R> TypedOperation<F, R> {
    pub const fn new(callback: F, response: oneshot::Sender<Result<R, RuntimeError>>) -> Self {
        Self { callback, response }
    }
}

impl<F, R> VmOperation for TypedOperation<F, R>
where
    F: FnOnce(&mut Vm) -> velum::Result<R> + Send + 'static,
    R: Send + 'static,
{
    fn execute(self: Box<Self>, vm: &mut Vm) {
        let Self { callback, response } = *self;
        let result = callback(vm).map_err(|error| RuntimeError::engine(&error));
        drop(response.send(result));
    }

    fn reject(self: Box<Self>, error: RuntimeError) {
        let Self { callback, response } = *self;
        drop(callback);
        drop(response.send(Err(error)));
    }
}

pub enum VmCommand {
    Run(Box<dyn VmOperation>),
    WaitIdle(oneshot::Sender<Result<(), RuntimeError>>),
}

/// A cloneable, thread-safe command handle for one worker-owned VM.
#[derive(Clone)]
pub struct VmHandle {
    pub(crate) sender: mpsc::Sender<VmCommand>,
}

impl VmHandle {
    /// Runs one synchronous closure on the VM's owning thread.
    ///
    /// Calls sent through one handle are serialized. The closure may start
    /// asynchronous JavaScript work, but must not retain the mutable VM
    /// reference after it returns.
    ///
    /// # Errors
    /// Returns a runtime error when the VM is closed or the closure's Velum
    /// operation fails.
    pub async fn run<F, R>(&self, callback: F) -> Result<R, RuntimeError>
    where
        F: FnOnce(&mut Vm) -> velum::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let (response, receiver) = oneshot::channel();
        let operation = TypedOperation::new(callback, response);
        self.sender
            .send(VmCommand::Run(Box::new(operation)))
            .await
            .map_err(|_error| RuntimeError::VmClosed)?;
        receiver
            .await
            .map_err(|_error| RuntimeError::ResponseDropped)?
    }

    /// Waits until the VM has no host futures, host commands, or Promise jobs.
    ///
    /// # Errors
    /// Returns the first background VM error or reports that the VM closed.
    pub async fn wait_idle(&self) -> Result<(), RuntimeError> {
        let (response, receiver) = oneshot::channel();
        self.sender
            .send(VmCommand::WaitIdle(response))
            .await
            .map_err(|_error| RuntimeError::VmClosed)?;
        receiver
            .await
            .map_err(|_error| RuntimeError::ResponseDropped)?
    }
}
