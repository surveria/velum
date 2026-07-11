use crate::{
    error::Result,
    runtime::{
        Context,
        object::PropertyLookup,
        property::{get_property, get_property_with_receiver, has_property},
    },
    value::{ObjectId, Value},
};

mod descriptor;
mod invocation;
mod keys;
mod mutation;
mod prototype_integrity;

pub(in crate::runtime) use prototype_integrity::SemanticIntegrityLevel;

/// Result of object-like `[[Get]]` pre-dispatch. Optimized callers may handle
/// the ordinary `ObjectHeap` tail, but all exotic dispatch is already resolved.
#[derive(Debug)]
pub(in crate::runtime) enum SemanticPropertyRead {
    Resolved(Value),
    ObjectTail(ObjectId),
}

/// Result of object-like `[[HasProperty]]` pre-dispatch. Optimized callers may
/// handle the ordinary `ObjectHeap` tail without repeating value-kind checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SemanticPropertyPresence {
    Resolved(bool),
    ObjectTail(ObjectId),
}

/// Result of object-like `[[Set]]` pre-dispatch. Optimized callers may write
/// only the ordinary `ObjectHeap` tail after exotic behavior is resolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SemanticPropertyWrite {
    Resolved(bool),
    ObjectTail(ObjectId),
}

/// Result of object-like `[[Delete]]` pre-dispatch. Optimized callers may
/// delete only from the returned ordinary `ObjectHeap` tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SemanticPropertyDelete {
    Resolved(bool),
    ObjectTail(ObjectId),
}

/// A value that has been checked against its current physical runtime owner.
///
/// This is an internal, call-local semantic boundary over the existing split
/// stores. Durable embedding values use the identity- and generation-checked
/// `RetainedValue` boundary instead of exposing this reference.
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
            | Value::HostFunction(_) => None,
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

    /// Runs the shared object-like `[[Get]]` dispatch and returns an ordinary
    /// object tail only when a cache or the generic heap path may finish it.
    pub(in crate::runtime) fn semantic_property_read(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<SemanticPropertyRead>> {
        self.semantic_property_read_with_receiver(object, object, property)
    }

    /// Runs shared object-like `[[Get]]` dispatch with an explicit receiver,
    /// as required by `Reflect.get` and inherited accessor evaluation.
    pub(in crate::runtime) fn semantic_property_read_with_receiver(
        &mut self,
        object: &Value,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<SemanticPropertyRead>> {
        let Some(object_ref) = self.semantic_object_ref(object)? else {
            return Ok(None);
        };
        let read = match object_ref.value {
            Value::Object(id) => {
                if self.objects.is_proxy(*id) {
                    SemanticPropertyRead::Resolved(self.proxy_get(
                        *id,
                        property,
                        receiver.clone(),
                    )?)
                } else if let Some(value) =
                    self.get_string_object_property_value(*id, property.name())?
                {
                    SemanticPropertyRead::Resolved(value)
                } else if let Some(value) = self.global_object_property_value(*id, property)? {
                    SemanticPropertyRead::Resolved(value)
                } else {
                    SemanticPropertyRead::ObjectTail(*id)
                }
            }
            Value::Function(id) => SemanticPropertyRead::Resolved(
                self.get_function_property_lookup(*id, receiver, property)?,
            ),
            Value::NativeFunction(id) => SemanticPropertyRead::Resolved(
                self.get_native_function_property_lookup(*id, property)?,
            ),
            Value::HostFunction(_) => {
                let value = get_property(&self.objects, object, property)?;
                SemanticPropertyRead::Resolved(self.runtime_property_value(value)?)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(read))
    }

    /// Finishes a shared object-like property read through the generic heap
    /// path after an optimizer declined the ordinary-object tail.
    pub(in crate::runtime) fn finish_semantic_property_read(
        &mut self,
        read: SemanticPropertyRead,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        match read {
            SemanticPropertyRead::Resolved(value) => Ok(value),
            SemanticPropertyRead::ObjectTail(id) => {
                let value = get_property_with_receiver(&self.objects, id, receiver, property)?;
                self.runtime_property_value(value)
            }
        }
    }

    /// Runs the shared object-like `[[HasProperty]]` dispatch and returns an
    /// ordinary object tail only when a cache or the heap may finish it.
    pub(in crate::runtime) fn semantic_property_presence(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<SemanticPropertyPresence>> {
        let Some(object_ref) = self.semantic_object_ref(object)? else {
            return Ok(None);
        };
        let presence = match object_ref.value {
            Value::Object(id) => {
                if self.objects.is_proxy(*id) {
                    SemanticPropertyPresence::Resolved(self.proxy_has(*id, property)?)
                } else if let Some(value) = self.global_object_has_property(*id, property)? {
                    SemanticPropertyPresence::Resolved(value)
                } else {
                    SemanticPropertyPresence::ObjectTail(*id)
                }
            }
            Value::Function(id) => SemanticPropertyPresence::Resolved(
                self.has_function_property_including_prototype_lookup(*id, property)?,
            ),
            Value::NativeFunction(id) => SemanticPropertyPresence::Resolved(
                self.has_native_function_property_lookup(*id, property)?,
            ),
            Value::HostFunction(_) => {
                SemanticPropertyPresence::Resolved(has_property(&self.objects, object, property)?)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(presence))
    }

    /// Finishes a shared object-like presence check after an optimizer
    /// declined the ordinary-object tail.
    pub(in crate::runtime) fn finish_semantic_property_presence(
        &self,
        presence: SemanticPropertyPresence,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        match presence {
            SemanticPropertyPresence::Resolved(value) => Ok(value),
            SemanticPropertyPresence::ObjectTail(id) => self.objects.has(id, property),
        }
    }
}
