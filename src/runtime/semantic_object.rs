use crate::{
    error::Result,
    runtime::Context,
    value::{ObjectId, Value},
};

/// A value that has been checked against its current physical runtime owner.
///
/// This is an incremental semantic boundary over the existing split stores.
/// It deliberately does not prove VM identity yet; AS-05 will add VM-bound,
/// generation-aware handles.
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) struct SemanticObjectRef<'value> {
    value: &'value Value,
}

impl SemanticObjectRef<'_> {
    /// Returns the current `ObjectHeap` slot when this semantic object uses one.
    pub(in crate::runtime) const fn object_id(self) -> Option<ObjectId> {
        match self.value {
            Value::Object(id) => Some(*id),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => None,
        }
    }
}

impl Context {
    /// Resolves every currently object-like `Value` variant through one checked
    /// entrypoint while leaving its physical payload in the existing store.
    pub(in crate::runtime) fn semantic_object_ref<'value>(
        &self,
        value: &'value Value,
    ) -> Result<Option<SemanticObjectRef<'value>>> {
        match value {
            Value::Object(id) => self.objects.validate_id(*id)?,
            Value::Function(id) => {
                self.function(*id)?;
            }
            Value::NativeFunction(id) => {
                self.native_function(*id)?;
            }
            Value::HostFunction(id) => self.validate_host_function_id(*id)?,
            Value::Error(_) => {}
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        }
        Ok(Some(SemanticObjectRef { value }))
    }
}
