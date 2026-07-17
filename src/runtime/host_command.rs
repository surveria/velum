use std::{
    collections::VecDeque,
    fmt,
    future::Future,
    pin::Pin,
    rc::{Rc, Weak},
    task::{Context as TaskContext, Poll, Waker},
};

use parking_lot::Mutex;

use crate::{
    HostFutureError, HostTaskResult, OwnedValue, RetainedValue,
    error::{Error, Result},
    ownership::VmIdentity,
    value::Value,
};

use super::{
    Context, VmRootKind, VmStorageKind,
    promise::{PromiseReaction, PromiseSettledState},
    storage_ledger::VmStorageLedger,
};

const INITIAL_HOST_COMMAND_GENERATION: u64 = 1;
const HOST_COMMAND_CANCELLED_MESSAGE: &str = "JavaScript host command was cancelled";
const HOST_COMMAND_TORN_DOWN_MESSAGE: &str = "JavaScript host command owner was torn down";
const HOST_COMMAND_STALE_MESSAGE: &str = "JavaScript host command request is stale";
const FOREIGN_HOST_COMMAND_CALLABLE_MESSAGE: &str =
    "JavaScript host command callable belongs to another VM";

/// VM-bound command sender available to an asynchronous Rust host function.
///
/// The sender never borrows or reenters the VM. It transfers an explicitly
/// retained callable and owned primitive arguments into a VM-local FIFO queue.
#[derive(Clone)]
pub struct HostAsyncContext {
    identity: VmIdentity,
    state: Weak<Mutex<HostCommandState>>,
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
        let Some(state) = self.state.upgrade() else {
            return Err(Error::runtime(HOST_COMMAND_TORN_DOWN_MESSAGE));
        };
        let id = state.lock().enqueue(callable, args)?;
        Ok(HostCommandRequest {
            target: HostCommandTarget {
                state: Rc::downgrade(&state),
                id,
            },
            active: true,
        })
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

/// Future for the primitive result of one queued JavaScript call.
///
/// Dropping the future abandons the request and releases its callable,
/// arguments, response, and storage accounting. A JavaScript rejection is
/// returned as [`HostFutureError::JavaScript`] with its original VM-local
/// value identity and root preserved.
#[must_use = "a queued JavaScript call does not complete unless its future is polled"]
pub struct HostCommandRequest {
    target: HostCommandTarget,
    active: bool,
}

impl Future for HostCommandRequest {
    type Output = HostTaskResult<OwnedValue>;

    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let request = self.get_mut();
        let result = request.target.poll(context);
        if result.is_ready() {
            request.active = false;
        }
        result
    }
}

impl fmt::Debug for HostCommandRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostCommandRequest")
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl Drop for HostCommandRequest {
    fn drop(&mut self) {
        if self.active {
            self.target.abandon();
            self.active = false;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostCommandId {
    index: usize,
    generation: u64,
}

#[derive(Clone)]
struct HostCommandTarget {
    state: Weak<Mutex<HostCommandState>>,
    id: HostCommandId,
}

impl HostCommandTarget {
    fn poll(&self, context: &TaskContext<'_>) -> Poll<HostTaskResult<OwnedValue>> {
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

pub(super) struct HostCommandQueue {
    identity: VmIdentity,
    state: Rc<Mutex<HostCommandState>>,
}

impl HostCommandQueue {
    pub(super) fn new(identity: VmIdentity, storage_ledger: VmStorageLedger) -> Self {
        Self {
            identity,
            state: Rc::new(Mutex::new(HostCommandState {
                entries: Vec::new(),
                queued: VecDeque::new(),
                active_count: 0,
                active_payload_bytes: 0,
                storage_ledger,
            })),
        }
    }

    fn sender(&self) -> HostAsyncContext {
        HostAsyncContext {
            identity: self.identity.clone(),
            state: Rc::downgrade(&self.state),
        }
    }

    fn take_next(&self) -> Result<Option<(HostCommandCompletion, HostCommand)>> {
        let next = self.state.lock().take_next()?;
        Ok(next.map(|(id, command)| {
            (
                HostCommandCompletion {
                    target: HostCommandTarget {
                        state: Rc::downgrade(&self.state),
                        id,
                    },
                },
                command,
            )
        }))
    }

    pub(super) fn active_count(&self) -> usize {
        self.state.lock().active_count
    }

    pub(super) fn active_payload_bytes(&self) -> usize {
        self.state.lock().active_payload_bytes
    }

    pub(super) fn queued_count(&self) -> usize {
        self.state.lock().queued.len()
    }

    fn cancel_all(&self) -> Result<usize> {
        let (cancelled, wakers) = self.state.lock().cancel_all()?;
        for waker in wakers {
            waker.wake();
        }
        Ok(cancelled)
    }
}

impl fmt::Debug for HostCommandQueue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostCommandQueue")
            .field("identity", &self.identity)
            .field("active_count", &self.active_count())
            .field("queued_count", &self.queued_count())
            .finish_non_exhaustive()
    }
}

struct HostCommandState {
    entries: Vec<HostCommandEntry>,
    queued: VecDeque<HostCommandId>,
    active_count: usize,
    active_payload_bytes: usize,
    storage_ledger: VmStorageLedger,
}

struct HostCommandEntry {
    generation: u64,
    status: Option<HostCommandStatus>,
    payload_bytes: usize,
    waker: Option<Waker>,
}

enum HostCommandStatus {
    Queued(HostCommand),
    Waiting,
    Ready(HostCommandResponse),
}

struct HostCommand {
    callable: RetainedValue,
    args: Vec<OwnedValue>,
}

enum HostCommandResponse {
    Fulfilled(OwnedValue),
    Rejected(RetainedValue),
    Failed(Error),
}

impl HostCommandResponse {
    fn payload_bytes(&self) -> Result<usize> {
        match self {
            Self::Fulfilled(value) => owned_value_payload_bytes(value),
            Self::Rejected(_) | Self::Failed(_) => Ok(0),
        }
    }

    fn into_result(self) -> HostTaskResult<OwnedValue> {
        match self {
            Self::Fulfilled(value) => Ok(value),
            Self::Rejected(value) => Err(HostFutureError::JavaScript(value)),
            Self::Failed(error) => Err(HostFutureError::Engine(error)),
        }
    }
}

impl HostCommandState {
    fn enqueue(&mut self, callable: RetainedValue, args: Vec<OwnedValue>) -> Result<HostCommandId> {
        let payload_bytes = owned_values_payload_bytes(&args)?;
        let projected_count = self
            .active_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("host command count overflowed"))?;
        let projected_payload_bytes = self
            .active_payload_bytes
            .checked_add(payload_bytes)
            .ok_or_else(|| Error::limit("host command payload bytes overflowed"))?;
        self.queued
            .try_reserve(1)
            .map_err(|_| Error::limit("host command queue capacity exceeded"))?;

        let reusable = self.entries.iter().enumerate().find_map(|(index, entry)| {
            (entry.status.is_none())
                .then(|| {
                    entry
                        .generation
                        .checked_add(1)
                        .map(|generation| (index, generation))
                })
                .flatten()
        });
        if reusable.is_none() {
            self.entries
                .try_reserve(1)
                .map_err(|_| Error::limit("host command registry capacity exceeded"))?;
        }
        let reservation =
            self.storage_ledger
                .reserve(VmStorageKind::HostCommand, 1, payload_bytes)?;
        reservation.commit()?;
        let id = if let Some((index, generation)) = reusable {
            let Some(entry) = self.entries.get_mut(index) else {
                self.storage_ledger
                    .release(VmStorageKind::HostCommand, 1, payload_bytes)?;
                return Err(Error::runtime("host command reusable slot disappeared"));
            };
            entry.generation = generation;
            entry.status = Some(HostCommandStatus::Queued(HostCommand { callable, args }));
            entry.payload_bytes = payload_bytes;
            entry.waker = None;
            HostCommandId { index, generation }
        } else {
            let index = self.entries.len();
            self.entries.push(HostCommandEntry {
                generation: INITIAL_HOST_COMMAND_GENERATION,
                status: Some(HostCommandStatus::Queued(HostCommand { callable, args })),
                payload_bytes,
                waker: None,
            });
            HostCommandId {
                index,
                generation: INITIAL_HOST_COMMAND_GENERATION,
            }
        };
        self.active_count = projected_count;
        self.active_payload_bytes = projected_payload_bytes;
        self.queued.push_back(id);
        Ok(id)
    }

    fn take_next(&mut self) -> Result<Option<(HostCommandId, HostCommand)>> {
        while let Some(id) = self.queued.pop_front() {
            let Some(entry) = self.entries.get(id.index) else {
                continue;
            };
            if entry.generation != id.generation
                || !matches!(entry.status, Some(HostCommandStatus::Queued(_)))
            {
                continue;
            }
            let payload_bytes = entry.payload_bytes;
            self.storage_ledger
                .release(VmStorageKind::HostCommand, 0, payload_bytes)?;
            self.active_payload_bytes = self
                .active_payload_bytes
                .checked_sub(payload_bytes)
                .ok_or_else(|| Error::runtime("host command payload accounting underflowed"))?;
            let Some(entry) = self.matching_entry_mut(id) else {
                return Err(Error::runtime("queued host command slot disappeared"));
            };
            entry.payload_bytes = 0;
            let Some(HostCommandStatus::Queued(command)) =
                entry.status.replace(HostCommandStatus::Waiting)
            else {
                return Err(Error::runtime("queued host command disappeared"));
            };
            return Ok(Some((id, command)));
        }
        Ok(None)
    }

    fn matching_entry_mut(&mut self, id: HostCommandId) -> Option<&mut HostCommandEntry> {
        self.entries
            .get_mut(id.index)
            .filter(|entry| entry.generation == id.generation)
    }

    fn poll_response(
        &mut self,
        id: HostCommandId,
        context: &TaskContext<'_>,
    ) -> Poll<HostTaskResult<OwnedValue>> {
        let Some(entry) = self.matching_entry_mut(id) else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                HOST_COMMAND_STALE_MESSAGE,
            ))));
        };
        if !matches!(entry.status, Some(HostCommandStatus::Ready(_))) {
            if entry.status.is_none() {
                return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                    HOST_COMMAND_STALE_MESSAGE,
                ))));
            }
            entry.waker = Some(context.waker().clone());
            return Poll::Pending;
        }
        let payload_bytes = entry.payload_bytes;
        let Some(active_count) = self.active_count.checked_sub(1) else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                "host command count accounting underflowed",
            ))));
        };
        let Some(active_payload_bytes) = self.active_payload_bytes.checked_sub(payload_bytes)
        else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                "host command payload accounting underflowed",
            ))));
        };
        if let Err(error) =
            self.storage_ledger
                .release(VmStorageKind::HostCommand, 1, payload_bytes)
        {
            return Poll::Ready(Err(HostFutureError::from(error)));
        }
        let Some(entry) = self.matching_entry_mut(id) else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                "ready host command slot disappeared",
            ))));
        };
        let Some(HostCommandStatus::Ready(response)) = entry.status.take() else {
            return Poll::Ready(Err(HostFutureError::from(Error::runtime(
                "host command response disappeared",
            ))));
        };
        entry.payload_bytes = 0;
        entry.waker = None;
        self.active_count = active_count;
        self.active_payload_bytes = active_payload_bytes;
        Poll::Ready(response.into_result())
    }

    fn complete(
        &mut self,
        id: HostCommandId,
        response: HostCommandResponse,
    ) -> Result<(bool, Option<Waker>)> {
        let Some(entry) = self.entries.get(id.index) else {
            return Ok((false, None));
        };
        if entry.generation != id.generation
            || !matches!(entry.status, Some(HostCommandStatus::Waiting))
        {
            return Ok((false, None));
        }
        let mut response = response;
        let mut payload_bytes = response.payload_bytes()?;
        let mut projected_payload_bytes = self
            .active_payload_bytes
            .checked_add(payload_bytes)
            .ok_or_else(|| Error::limit("host command payload bytes overflowed"))?;
        let reservation = self
            .storage_ledger
            .reserve(VmStorageKind::HostCommand, 0, payload_bytes);
        if let Err(error) = reservation {
            response = HostCommandResponse::Failed(error);
            payload_bytes = 0;
            projected_payload_bytes = self.active_payload_bytes;
        } else if let Ok(reservation) = reservation {
            reservation.commit()?;
        }
        let Some(entry) = self.matching_entry_mut(id) else {
            self.storage_ledger
                .release(VmStorageKind::HostCommand, 0, payload_bytes)?;
            return Err(Error::runtime("waiting host command slot disappeared"));
        };
        entry.payload_bytes = payload_bytes;
        entry.status = Some(HostCommandStatus::Ready(response));
        let waker = entry.waker.take();
        self.active_payload_bytes = projected_payload_bytes;
        Ok((true, waker))
    }

    fn cancel_all(&mut self) -> Result<(usize, Vec<Waker>)> {
        let (cancelled, released_payload_bytes, waker_count) = self.entries.iter().try_fold(
            (0_usize, 0_usize, 0_usize),
            |(count, payload_bytes, wakers), entry| {
                let Some(status) = &entry.status else {
                    return Ok((count, payload_bytes, wakers));
                };
                if matches!(status, HostCommandStatus::Ready(_)) {
                    return Ok((count, payload_bytes, wakers));
                }
                Ok((
                    count.checked_add(1).ok_or_else(|| {
                        Error::limit("host command cancellation count overflowed")
                    })?,
                    payload_bytes
                        .checked_add(entry.payload_bytes)
                        .ok_or_else(|| {
                            Error::limit("host command cancellation payload overflowed")
                        })?,
                    wakers
                        .checked_add(usize::from(entry.waker.is_some()))
                        .ok_or_else(|| Error::limit("host command waker count overflowed"))?,
                ))
            },
        )?;
        let projected_payload_bytes = self
            .active_payload_bytes
            .checked_sub(released_payload_bytes)
            .ok_or_else(|| Error::runtime("host command payload accounting underflowed"))?;
        let mut wakers = Vec::new();
        wakers
            .try_reserve_exact(waker_count)
            .map_err(|_| Error::limit("host command cancellation waker capacity exceeded"))?;
        self.storage_ledger
            .release(VmStorageKind::HostCommand, 0, released_payload_bytes)?;
        self.queued.clear();
        for entry in &mut self.entries {
            let Some(status) = &entry.status else {
                continue;
            };
            if matches!(status, HostCommandStatus::Ready(_)) {
                continue;
            }
            entry.payload_bytes = 0;
            entry.status = Some(HostCommandStatus::Ready(HostCommandResponse::Failed(
                Error::runtime(HOST_COMMAND_CANCELLED_MESSAGE),
            )));
            if let Some(waker) = entry.waker.take() {
                wakers.push(waker);
            }
        }
        self.active_payload_bytes = projected_payload_bytes;
        Ok((cancelled, wakers))
    }

    fn abandon(&mut self, id: HostCommandId) {
        let Some(entry) = self.matching_entry_mut(id) else {
            return;
        };
        if entry.status.take().is_none() {
            return;
        }
        let payload_bytes = entry.payload_bytes;
        entry.payload_bytes = 0;
        entry.waker = None;
        self.queued.retain(|queued| *queued != id);
        self.storage_ledger
            .release_on_drop(VmStorageKind::HostCommand, 1, payload_bytes);
        self.active_count = self.active_count.saturating_sub(1);
        self.active_payload_bytes = self.active_payload_bytes.saturating_sub(payload_bytes);
    }
}

/// Delivery target owned by an ordinary Promise reaction.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct HostCommandCompletion {
    target: HostCommandTarget,
}

impl HostCommandCompletion {
    fn complete(self, response: HostCommandResponse) -> Result<()> {
        let Some(state) = self.target.state.upgrade() else {
            return Ok(());
        };
        let (_accepted, waker) = state.lock().complete(self.target.id, response)?;
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(())
    }

    pub(super) fn cancel(self) -> Result<()> {
        self.complete(HostCommandResponse::Failed(Error::runtime(
            HOST_COMMAND_CANCELLED_MESSAGE,
        )))
    }
}

impl Context {
    pub(crate) fn host_async_context(&self) -> HostAsyncContext {
        self.host_commands.sender()
    }

    /// Runs every currently queued Rust-to-JavaScript command in FIFO order.
    ///
    /// Each call is dispatched through the shared semantic call owner. Its
    /// result is adopted as a Promise and delivered back to the Rust future by
    /// the ordinary Promise job queue; this method does not run those jobs.
    ///
    /// # Errors
    /// Fails when queue accounting or Promise-reaction admission fails.
    pub fn run_host_commands(&mut self) -> Result<usize> {
        let mut executed = 0_usize;
        while let Some((completion, command)) = self.take_host_command()? {
            self.start_host_command(completion, command)?;
            executed = executed
                .checked_add(1)
                .ok_or_else(|| Error::limit("host command execution count overflowed"))?;
        }
        Ok(executed)
    }

    /// Returns all queued or awaiting-result JavaScript host commands.
    #[must_use]
    pub fn pending_host_command_count(&self) -> usize {
        self.host_commands.active_count()
    }

    /// Returns commands that have not yet entered JavaScript execution.
    #[must_use]
    pub fn queued_host_command_count(&self) -> usize {
        self.host_commands.queued_count()
    }

    /// Cancels every queued or awaiting-result JavaScript host command.
    ///
    /// # Errors
    /// Fails when storage accounting cannot be reconciled.
    pub fn cancel_host_commands(&mut self) -> Result<usize> {
        self.host_commands.cancel_all()
    }

    pub(in crate::runtime) fn settle_host_command_reaction(
        &self,
        completion: HostCommandCompletion,
        state: PromiseSettledState,
    ) -> Result<()> {
        let response = match state.into_completion() {
            super::control::Completion::Throw(reason) => match self.retain_embedder_value(reason) {
                Ok(reason) => HostCommandResponse::Rejected(reason),
                Err(error) => HostCommandResponse::Failed(error),
            },
            super::control::Completion::Normal(value) => match OwnedValue::try_from(value) {
                Ok(value) => HostCommandResponse::Fulfilled(value),
                Err(error) => HostCommandResponse::Failed(error),
            },
            _ => HostCommandResponse::Failed(Error::runtime(
                "host command Promise reaction produced an invalid completion",
            )),
        };
        completion.complete(response)
    }

    fn take_host_command(&self) -> Result<Option<(HostCommandCompletion, HostCommand)>> {
        self.host_commands.take_next()
    }

    fn start_host_command(
        &mut self,
        completion: HostCommandCompletion,
        command: HostCommand,
    ) -> Result<()> {
        let callable = match self.resolve_retained_value(&command.callable) {
            Ok(value) => value,
            Err(error) => return completion.complete(HostCommandResponse::Failed(error)),
        };
        let mut args = Vec::new();
        if args.try_reserve(command.args.len()).is_err() {
            return completion.complete(HostCommandResponse::Failed(Error::limit(
                "host command argument capacity exceeded",
            )));
        }
        for argument in command.args {
            match self.runtime_value(argument.into()) {
                Ok(value) => args.push(value),
                Err(error) => return completion.complete(HostCommandResponse::Failed(error)),
            }
        }
        let _roots = match self.transient_root_scope(VmRootKind::TransientCall, args.iter()) {
            Ok(roots) => roots,
            Err(error) => return completion.complete(HostCommandResponse::Failed(error)),
        };
        let value = match self.embedding_call(&callable, &args, Value::Undefined) {
            Ok(value) => value,
            Err(error) => {
                return self.complete_host_command_error(completion, error);
            }
        };
        let _result_root =
            match self.transient_root_scope(VmRootKind::TransientCall, std::iter::once(&value)) {
                Ok(root) => root,
                Err(error) => return completion.complete(HostCommandResponse::Failed(error)),
            };
        let promise = match self.promise_resolve_for_await(value) {
            Ok(promise) => promise,
            Err(error) => return self.complete_host_command_error(completion, error),
        };
        let rollback = completion.clone();
        if let Err(error) =
            self.add_promise_reaction(promise, PromiseReaction::host_command(completion))
        {
            return rollback.complete(HostCommandResponse::Failed(error));
        }
        Ok(())
    }

    fn complete_host_command_error(
        &self,
        completion: HostCommandCompletion,
        error: Error,
    ) -> Result<()> {
        let response = if error.javascript_identity() == Some(self.identity()) {
            let Some(value) = error.javascript_value() else {
                return completion.complete(HostCommandResponse::Failed(error));
            };
            match self.retain_embedder_value(value.clone()) {
                Ok(value) => HostCommandResponse::Rejected(value),
                Err(error) => HostCommandResponse::Failed(error),
            }
        } else {
            HostCommandResponse::Failed(error)
        };
        completion.complete(response)
    }
}

fn owned_values_payload_bytes(values: &[OwnedValue]) -> Result<usize> {
    values.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(owned_value_payload_bytes(value)?)
            .ok_or_else(|| Error::limit("host command payload bytes overflowed"))
    })
}

fn owned_value_payload_bytes(value: &OwnedValue) -> Result<usize> {
    match value {
        OwnedValue::String(value) => Ok(value.len()),
        OwnedValue::BigInt(value) => {
            let bytes = value.bit_len().saturating_add(7) / 8;
            usize::try_from(bytes).map_err(|_| Error::limit("host command BigInt size overflowed"))
        }
        OwnedValue::Undefined | OwnedValue::Null | OwnedValue::Bool(_) | OwnedValue::Number(_) => {
            Ok(0)
        }
    }
}
