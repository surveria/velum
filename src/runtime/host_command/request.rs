use std::{
    fmt,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context as TaskContext, Poll},
};

use parking_lot::Mutex;

use crate::{
    HostFutureError, HostTaskResult, OwnedValue, RetainedValue,
    error::{Error, Result},
    ownership::VmIdentity,
};

use super::{
    FOREIGN_HOST_COMMAND_CALLABLE_MESSAGE, HOST_COMMAND_TORN_DOWN_MESSAGE, HostCommand,
    HostCommandId, HostCommandState, HostCommandValue,
};

const VM_LOCAL_HOST_COMMAND_RESULT_MESSAGE: &str =
    "JavaScript host command returned a VM-local value; use Vm::enqueue_call for retained results";

/// VM-bound command sender available to an asynchronous Rust host function.
///
/// The sender never borrows or reenters the VM. It transfers an explicitly
/// retained callable and owned primitive arguments into a VM-local FIFO queue.
#[derive(Clone)]
pub struct HostAsyncContext {
    pub(super) identity: VmIdentity,
    pub(super) state: Weak<Mutex<HostCommandState>>,
}

impl HostAsyncContext {
    /// Queues one JavaScript call with `undefined` as its receiver.
    ///
    /// The returned future completes only after [`crate::Vm::run_host_commands`]
    /// invokes the callable and the ordinary Promise job queue delivers its
    /// synchronous or asynchronous result.
    ///
    /// # Errors
    /// Fails for a foreign callable, a torn-down VM, exhausted queue storage,
    /// or configured host-command limits.
    pub fn call(
        &self,
        callable: RetainedValue,
        args: Vec<OwnedValue>,
    ) -> Result<HostCommandRequest> {
        if callable.identity() != &self.identity {
            return Err(Error::runtime(FOREIGN_HOST_COMMAND_CALLABLE_MESSAGE));
        }
        let mut command_args = Vec::new();
        command_args
            .try_reserve(args.len())
            .map_err(|_| Error::limit("host command argument capacity exceeded"))?;
        command_args.extend(args.into_iter().map(HostCommandValue::Owned));
        let command = HostCommand {
            callable,
            receiver: HostCommandValue::Owned(OwnedValue::Undefined),
            args: command_args,
        };
        let core = HostCommandRequestCore::enqueue(&self.state, command)?;
        Ok(HostCommandRequest { core })
    }

    /// Returns the VM generation that owns this command queue.
    #[must_use]
    pub const fn identity(&self) -> &VmIdentity {
        &self.identity
    }
}

impl fmt::Debug for HostAsyncContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostAsyncContext")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Result of a general queued JavaScript call.
///
/// Portable primitives are copied into [`OwnedValue`]. Symbols, objects,
/// functions, and exact strings that cannot be represented as UTF-8 remain
/// rooted in their owning VM as [`RetainedValue`].
#[derive(Debug)]
pub enum QueuedCallResult {
    /// VM-independent primitive copied out of JavaScript storage.
    Owned(OwnedValue),
    /// VM-local value kept alive by an explicit retained root.
    Retained(RetainedValue),
}

impl QueuedCallResult {
    pub(super) fn payload_bytes(&self) -> Result<usize> {
        match self {
            Self::Owned(value) => super::owned_value_payload_bytes(value),
            Self::Retained(_) => Ok(0),
        }
    }
}

/// Future for the primitive result of a call queued by an async Rust host task.
///
/// Dropping the future abandons the request and releases its callable,
/// arguments, response, and storage accounting. Use [`QueuedCallRequest`] for
/// embedder calls that may fulfil with a VM-local value.
#[must_use = "a queued JavaScript call does not complete unless its future is polled"]
pub struct HostCommandRequest {
    core: HostCommandRequestCore,
}

impl Future for HostCommandRequest {
    type Output = HostTaskResult<OwnedValue>;

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        match self.get_mut().core.poll(context) {
            Poll::Ready(Ok(QueuedCallResult::Owned(value))) => Poll::Ready(Ok(value)),
            Poll::Ready(Ok(QueuedCallResult::Retained(_))) => Poll::Ready(Err(
                HostFutureError::from(Error::runtime(VM_LOCAL_HOST_COMMAND_RESULT_MESSAGE)),
            )),
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl fmt::Debug for HostCommandRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostCommandRequest")
            .field("active", &self.core.active)
            .finish_non_exhaustive()
    }
}

/// Future for an embedder-queued JavaScript call and its adopted Promise result.
///
/// Dropping the request before polling it ready abandons the queued or waiting
/// call and releases every duplicated input and result root.
#[must_use = "an embedder-queued JavaScript call must be retained and polled"]
pub struct QueuedCallRequest {
    core: HostCommandRequestCore,
}

impl QueuedCallRequest {
    pub(super) fn enqueue(
        state: &Rc<Mutex<HostCommandState>>,
        command: HostCommand,
    ) -> Result<Self> {
        let weak = Rc::downgrade(state);
        Ok(Self {
            core: HostCommandRequestCore::enqueue(&weak, command)?,
        })
    }
}

impl Future for QueuedCallRequest {
    type Output = HostTaskResult<QueuedCallResult>;

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.get_mut().core.poll(context)
    }
}

impl fmt::Debug for QueuedCallRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QueuedCallRequest")
            .field("active", &self.core.active)
            .finish_non_exhaustive()
    }
}

struct HostCommandRequestCore {
    target: HostCommandTarget,
    active: bool,
}

impl HostCommandRequestCore {
    fn enqueue(state: &Weak<Mutex<HostCommandState>>, command: HostCommand) -> Result<Self> {
        let Some(state) = state.upgrade() else {
            return Err(Error::runtime(HOST_COMMAND_TORN_DOWN_MESSAGE));
        };
        let id = state.lock().enqueue(command)?;
        Ok(Self {
            target: HostCommandTarget {
                state: Rc::downgrade(&state),
                id,
            },
            active: true,
        })
    }

    fn poll(&mut self, context: &TaskContext<'_>) -> Poll<HostTaskResult<QueuedCallResult>> {
        let result = self.target.poll(context);
        if result.is_ready() {
            self.active = false;
        }
        result
    }
}

impl Drop for HostCommandRequestCore {
    fn drop(&mut self) {
        if self.active {
            self.target.abandon();
            self.active = false;
        }
    }
}

#[derive(Clone)]
pub(super) struct HostCommandTarget {
    pub(super) state: Weak<Mutex<HostCommandState>>,
    pub(super) id: HostCommandId,
}

impl HostCommandTarget {
    fn poll(&self, context: &TaskContext<'_>) -> Poll<HostTaskResult<QueuedCallResult>> {
        let Some(state) = self.state.upgrade() else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                HOST_COMMAND_TORN_DOWN_MESSAGE,
            ))));
        };
        state.lock().poll_response(self.id, context)
    }

    fn abandon(&self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        state.lock().abandon(self.id);
    }
}

impl fmt::Debug for HostCommandTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostCommandTarget")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
