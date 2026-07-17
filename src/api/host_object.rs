use crate::{RetainedValue, api::embedding::Vm, error::Result};

use super::object::ObjectPrototypeOption;

/// Creation policy for an ordinary JavaScript object with a typed Rust payload.
#[derive(Clone, Copy, Debug)]
pub struct HostObjectOptions<'value> {
    logical_payload_bytes: usize,
    prototype: ObjectPrototypeOption<'value>,
    traced_values: &'value [RetainedValue],
}

impl HostObjectOptions<'_> {
    /// Creates options with the logical payload size charged once regardless
    /// of how many JavaScript wrappers explicitly share the payload.
    #[must_use]
    pub const fn new(logical_payload_bytes: usize) -> Self {
        Self {
            logical_payload_bytes,
            prototype: ObjectPrototypeOption::Default,
            traced_values: &[],
        }
    }
}

impl<'value> HostObjectOptions<'value> {
    /// Uses this VM-local object or retained `null` as the wrapper's
    /// `[[Prototype]]`.
    #[must_use]
    pub const fn with_prototype(mut self, prototype: &'value RetainedValue) -> Self {
        self.prototype = ObjectPrototypeOption::Explicit(prototype);
        self
    }

    /// Traces these VM-local values as internal edges from every wrapper.
    ///
    /// The values are not promoted to independent direct roots, so cycles
    /// containing the host wrapper remain collectible.
    #[must_use]
    pub const fn with_traced_values(mut self, values: &'value [RetainedValue]) -> Self {
        self.traced_values = values;
        self
    }
}

impl HostObjectOptions<'_> {
    /// Creates a wrapper whose `[[Prototype]]` is `null`.
    #[must_use]
    pub const fn with_null_prototype(mut self) -> Self {
        self.prototype = ObjectPrototypeOption::Null;
        self
    }
}

impl Vm {
    /// Creates an ordinary JavaScript object with an opaque typed Rust payload.
    ///
    /// JavaScript property and prototype behavior remains owned by the ordinary
    /// object implementation. Payload mutation should use explicit interior
    /// synchronization appropriate to the application.
    ///
    /// # Errors
    /// Fails for foreign option handles, invalid prototypes, object, payload,
    /// instance, edge, or retained-root limits, or VM storage failures.
    pub fn create_host_object<T: 'static>(
        &mut self,
        payload: T,
        options: HostObjectOptions<'_>,
    ) -> Result<RetainedValue> {
        self.embedding_context_mut().create_typed_host_object(
            payload,
            options.logical_payload_bytes,
            options.prototype.into_embedding(),
            options.traced_values,
        )
    }

    /// Creates a distinct ordinary wrapper that explicitly shares a payload.
    ///
    /// Own JavaScript properties are not copied. The new wrapper receives the
    /// source wrapper's current prototype and shares its typed payload and
    /// traced edge set.
    ///
    /// # Errors
    /// Fails for foreign, stale, non-object, or non-host values, configured
    /// instance/object/root limits, or VM storage failures.
    pub fn clone_host_object(&mut self, source: &RetainedValue) -> Result<RetainedValue> {
        self.embedding_context_mut().clone_typed_host_object(source)
    }

    /// Borrows a checked typed payload for the lifetime of this VM borrow.
    ///
    /// # Errors
    /// Fails for foreign, stale, non-object, non-host, or mismatched payload
    /// types.
    pub fn host_payload<T: 'static>(&self, object: &RetainedValue) -> Result<&T> {
        self.embedding_context_ref().typed_host_payload(object)
    }

    /// Replaces the logical byte charge shared by all wrappers of one payload.
    ///
    /// Applications should call this transactionally when variable-size state
    /// owned by the payload changes. External resources referenced by a fixed
    /// handle remain application-owned and need not be charged as VM memory.
    ///
    /// # Errors
    /// Fails for invalid host objects, accounting overflow, or the configured
    /// host-payload byte limit.
    pub fn update_host_payload_bytes(
        &mut self,
        object: &RetainedValue,
        logical_payload_bytes: usize,
    ) -> Result<()> {
        self.embedding_context_mut()
            .update_typed_host_payload_bytes(object, logical_payload_bytes)
    }
}
