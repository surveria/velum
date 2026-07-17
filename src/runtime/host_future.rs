use std::{fmt, task::Context as TaskContext, task::Poll};

use crate::{
    HostFuture, OwnedValue,
    error::{Error, JavaScriptErrorMetadata, Result},
    value::{ErrorName, Value},
};

use super::{
    Context, VmRootKind, VmStorageKind, promise::PromiseId, roots::DirectRootVisitor,
    storage_ledger::VmStorageReservation,
};

const HOST_FUTURE_CANCELLED_MESSAGE: &str = "async host function was cancelled";

/// Result of polling the VM-owned set of Rust host futures once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostFuturePoll {
    completed: usize,
    pending: usize,
}

impl HostFuturePoll {
    /// Returns the number of futures completed by this poll pass.
    #[must_use]
    pub const fn completed(self) -> usize {
        self.completed
    }

    /// Returns the number of futures still waiting after this poll pass.
    #[must_use]
    pub const fn pending(self) -> usize {
        self.pending
    }

    /// Returns whether every host future has completed.
    #[must_use]
    pub const fn is_idle(self) -> bool {
        self.pending == 0
    }
}

pub struct HostFutureAdmission {
    storage: VmStorageReservation,
}

pub struct PendingHostFuture {
    promise: PromiseId,
    future: HostFuture,
}

impl fmt::Debug for PendingHostFuture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingHostFuture")
            .field("promise", &self.promise)
            .finish_non_exhaustive()
    }
}

impl Context {
    pub(crate) fn prepare_host_future(&mut self) -> Result<HostFutureAdmission> {
        self.host_futures
            .try_reserve(1)
            .map_err(|_| Error::limit("host future capacity exceeded"))?;
        let storage = self
            .storage_ledger
            .reserve_count(VmStorageKind::HostFuture, 1)?;
        Ok(HostFutureAdmission { storage })
    }

    pub(crate) fn activate_host_future(
        &mut self,
        admission: HostFutureAdmission,
        future: HostFuture,
    ) -> Result<Value> {
        let (promise, object) = self.create_pending_promise()?;
        admission.storage.commit()?;
        self.host_futures
            .push(PendingHostFuture { promise, future });
        Ok(object)
    }

    pub(crate) fn create_rejected_host_future(&mut self, error: &Error) -> Result<Value> {
        let reason = self.host_future_error_value(error)?;
        self.create_rejected_promise(reason)
    }

    /// Polls each pending Rust host future once in FIFO creation order.
    ///
    /// Completed results settle their existing JavaScript Promises. Promise
    /// reactions are appended to the ordinary VM job queue and are not run by
    /// this method.
    ///
    /// # Errors
    /// Fails when temporary polling storage, result admission, Promise
    /// settlement, or accounting reconciliation fails.
    pub fn poll_host_futures(
        &mut self,
        task_context: &mut TaskContext<'_>,
    ) -> Result<HostFuturePoll> {
        let active = std::mem::take(&mut self.host_futures);
        let mut pending = Vec::new();
        if pending.try_reserve_exact(active.len()).is_err() {
            self.host_futures = active;
            return Err(Error::limit(
                "pending host future polling capacity exceeded",
            ));
        }
        let mut completed = Vec::new();
        if completed.try_reserve_exact(active.len()).is_err() {
            self.host_futures = active;
            return Err(Error::limit(
                "completed host future polling capacity exceeded",
            ));
        }

        for mut record in active {
            match record.future.as_mut().poll(task_context) {
                Poll::Pending => pending.push(record),
                Poll::Ready(result) => completed.push((record.promise, result)),
            }
        }
        self.host_futures = pending;
        self.storage_ledger
            .release_count(VmStorageKind::HostFuture, completed.len())?;

        let completed_count = completed.len();
        let mut first_error = None;
        for (promise, result) in completed {
            if self.active_host_future_promise.replace(promise).is_some() {
                return Err(Error::runtime(
                    "active host future Promise root was already occupied",
                ));
            }
            let settlement = self.settle_host_future(promise, result);
            self.active_host_future_promise = None;
            if let Err(error) = settlement
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(HostFuturePoll {
            completed: completed_count,
            pending: self.host_futures.len(),
        })
    }

    /// Returns the number of Rust futures awaiting completion.
    #[must_use]
    pub const fn pending_host_future_count(&self) -> usize {
        self.host_futures.len()
    }

    /// Drops all pending Rust host futures and rejects their Promises.
    ///
    /// Promise rejection handlers remain queued for [`Self::run_jobs`].
    ///
    /// # Errors
    /// Fails when the cancellation reason cannot be allocated, a Promise
    /// cannot be rejected, or accounting reconciliation fails.
    pub fn cancel_host_futures(&mut self) -> Result<usize> {
        if self.host_futures.is_empty() {
            return Ok(0);
        }
        let reason = self.create_error_object(
            JavaScriptErrorMetadata::new(ErrorName::Base, HOST_FUTURE_CANCELLED_MESSAGE),
            true,
        )?;
        let futures = std::mem::take(&mut self.host_futures);
        let count = futures.len();
        self.storage_ledger
            .release_count(VmStorageKind::HostFuture, count)?;

        let mut first_error = None;
        for future in futures {
            if self
                .active_host_future_promise
                .replace(future.promise)
                .is_some()
            {
                return Err(Error::runtime(
                    "active host future Promise root was already occupied",
                ));
            }
            let rejection = self.reject_promise(future.promise, reason.clone());
            self.active_host_future_promise = None;
            if let Err(error) = rejection
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(count)
    }

    pub(in crate::runtime) fn visit_host_future_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for future in &self.host_futures {
            visitor.visit_promise(VmRootKind::HostFuture, future.promise)?;
        }
        if let Some(promise) = self.active_host_future_promise {
            visitor.visit_promise(VmRootKind::HostFuture, promise)?;
        }
        Ok(())
    }

    fn settle_host_future(&mut self, promise: PromiseId, result: Result<OwnedValue>) -> Result<()> {
        match result {
            Ok(value) => match self.runtime_value(value.into()) {
                Ok(value) => self.resolve_promise(promise, value),
                Err(error) => self.reject_host_future_error(promise, &error),
            },
            Err(error) => self.reject_host_future_error(promise, &error),
        }
    }

    fn reject_host_future_error(&mut self, promise: PromiseId, error: &Error) -> Result<()> {
        let reason = self.host_future_error_value(error)?;
        self.reject_promise(promise, reason)
    }

    fn host_future_error_value(&mut self, error: &Error) -> Result<Value> {
        if let Some(metadata) = error
            .javascript_error_request()
            .or_else(|| error.javascript_error_metadata())
        {
            return self.create_error_object(metadata.clone(), true);
        }
        let name = match error {
            Error::Lex { .. } | Error::Parse { .. } => ErrorName::SyntaxError,
            Error::ResourceLimit { .. } => ErrorName::RangeError,
            Error::Runtime { .. } | Error::JavaScript { .. } | Error::JavaScriptError { .. } => {
                ErrorName::Base
            }
        };
        self.create_error_object(JavaScriptErrorMetadata::new(name, error.to_string()), true)
    }
}
