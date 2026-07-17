use crate::{
    JsValueRef, OwnedValue, RetainedValue,
    error::{Error, Result},
};

use super::{HostCommand, HostCommandValue, QueuedCallRequest};
use crate::runtime::Context;

const QUEUED_ARGUMENT_CAPACITY_ERROR: &str = "queued call argument capacity exceeded";
const QUEUED_STRING_CAPACITY_ERROR: &str = "queued call string capacity exceeded";

impl Context {
    pub(crate) fn enqueue_external_call(
        &mut self,
        callable: &RetainedValue,
        receiver: JsValueRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<QueuedCallRequest> {
        let callable = self.duplicate_command_root(callable)?;
        let receiver = self.command_value(receiver)?;
        let mut command_args = Vec::new();
        command_args
            .try_reserve(args.len())
            .map_err(|_| Error::limit(QUEUED_ARGUMENT_CAPACITY_ERROR))?;
        for argument in args {
            command_args.push(self.command_value(*argument)?);
        }
        self.host_commands.enqueue_external(HostCommand {
            callable,
            receiver,
            args: command_args,
        })
    }

    fn command_value(&mut self, value: JsValueRef<'_>) -> Result<HostCommandValue> {
        match value {
            JsValueRef::Undefined => Ok(HostCommandValue::Owned(OwnedValue::Undefined)),
            JsValueRef::Null => Ok(HostCommandValue::Owned(OwnedValue::Null)),
            JsValueRef::Bool(value) => Ok(HostCommandValue::Owned(OwnedValue::Bool(value))),
            JsValueRef::Number(value) => Ok(HostCommandValue::Owned(OwnedValue::Number(value))),
            JsValueRef::BigInt(value) => {
                Ok(HostCommandValue::Owned(OwnedValue::BigInt(value.clone())))
            }
            JsValueRef::String(value) => queued_string(value)
                .map(OwnedValue::String)
                .map(HostCommandValue::Owned),
            JsValueRef::ExactString(value) => {
                let value = self.heap_js_string_value(value)?;
                self.retain_embedder_value(value)
                    .map(HostCommandValue::Retained)
            }
            JsValueRef::Owned(value) => Ok(HostCommandValue::Owned(value.clone())),
            JsValueRef::Retained(value) => self
                .duplicate_command_root(value)
                .map(HostCommandValue::Retained),
        }
    }

    fn duplicate_command_root(&self, value: &RetainedValue) -> Result<RetainedValue> {
        let value = self.resolve_retained_value(value)?;
        self.retain_embedder_value(value)
    }
}

fn queued_string(value: &str) -> Result<String> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| Error::limit(QUEUED_STRING_CAPACITY_ERROR))?;
    owned.push_str(value);
    Ok(owned)
}
