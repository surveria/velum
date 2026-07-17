use crate::{
    JsValueRef, RetainedValue, api::embedding::Vm, error::Result, runtime::QueuedCallRequest,
};

impl Vm {
    /// Queues a retained JavaScript callable with `undefined` as its receiver.
    ///
    /// The returned future adopts synchronous and Promise results through the
    /// ordinary VM Promise owner. The application explicitly drives command
    /// dispatch with [`Self::run_host_commands`] and Promise reactions with
    /// [`Self::run_jobs`].
    ///
    /// # Errors
    /// Fails for a foreign or stale handle, input conversion or root admission
    /// failure, exhausted queue storage, or configured resource limits.
    pub fn enqueue_call(
        &mut self,
        callable: &RetainedValue,
        args: &[JsValueRef<'_>],
    ) -> Result<QueuedCallRequest> {
        self.enqueue_call_with_receiver(callable, JsValueRef::Undefined, args)
    }

    /// Queues a retained JavaScript callable with an explicit receiver.
    ///
    /// Borrowed retained inputs are duplicated into independent queue roots,
    /// so the application can keep and reuse its original callback handle for
    /// later events. Dispatch still enters the shared semantic call owner.
    ///
    /// # Errors
    /// Fails for a foreign or stale handle, input conversion or root admission
    /// failure, exhausted queue storage, or configured resource limits.
    pub fn enqueue_call_with_receiver(
        &mut self,
        callable: &RetainedValue,
        receiver: JsValueRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<QueuedCallRequest> {
        self.embedding_context_mut()
            .enqueue_external_call(callable, receiver, args)
    }
}
