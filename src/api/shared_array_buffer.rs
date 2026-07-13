use std::sync::Arc;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{ByteBuffer, SharedByteBuffer},
    },
    syntax::DeclKind,
};

const SHARED_BUFFER_ERROR: &str = "value is not a SharedArrayBuffer";

/// An opaque, thread-safe handle to one `SharedArrayBuffer` backing store.
///
/// Cloning the handle preserves storage identity. Embedders can install the
/// same handle in independent VMs without sharing any other JavaScript state.
#[derive(Clone, Debug)]
pub struct SharedArrayBufferHandle {
    shared: Arc<SharedByteBuffer>,
}

impl SharedArrayBufferHandle {
    pub(crate) fn from_buffer(buffer: &ByteBuffer) -> Result<Self> {
        let Some(shared) = buffer.shared_storage() else {
            return Err(Error::type_error(SHARED_BUFFER_ERROR));
        };
        Ok(Self { shared })
    }

    /// Returns the current shared backing-store length in bytes.
    #[must_use]
    pub fn byte_length(&self) -> usize {
        self.buffer().byte_length()
    }

    pub(crate) fn buffer(&self) -> ByteBuffer {
        ByteBuffer::from_shared_storage(self.shared.clone())
    }
}

impl Context {
    /// Installs a VM-local `SharedArrayBuffer` object backed by `handle` as a
    /// constant global binding.
    ///
    /// # Errors
    /// Fails when the binding is invalid or already exists, or when VM storage
    /// limits prevent creation of the local wrapper object.
    pub fn register_shared_array_buffer(
        &mut self,
        name: &str,
        handle: &SharedArrayBufferHandle,
    ) -> Result<()> {
        if name.is_empty() {
            return Err(Error::runtime(
                "shared ArrayBuffer binding name must not be empty",
            ));
        }
        let value = self.create_shared_array_buffer_value(handle.buffer())?;
        self.define(name, value, DeclKind::Const)
    }
}
