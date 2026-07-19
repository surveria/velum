#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    RetainedValue,
    api::host::HostFunction,
    error::{Error, Result},
    runtime::Context,
    value::{HostFunctionId, ObjectId, Value},
};

pub(super) struct StagedHostClass {
    prototype: RetainedValue,
    prototype_id: ObjectId,
    functions: Vec<RetainedValue>,
    function_ids: Vec<HostFunctionId>,
}

impl StagedHostClass {
    pub(super) fn allocate_function_storage(
        function_count: usize,
    ) -> Result<(Vec<RetainedValue>, Vec<HostFunctionId>)> {
        let mut functions = Vec::new();
        functions
            .try_reserve(function_count)
            .map_err(|_| Error::limit("host class function root capacity exceeded"))?;
        let mut function_ids = Vec::new();
        function_ids
            .try_reserve(function_count)
            .map_err(|_| Error::limit("host class function id capacity exceeded"))?;
        Ok((functions, function_ids))
    }

    pub(super) const fn new(
        prototype: RetainedValue,
        prototype_id: ObjectId,
        functions: Vec<RetainedValue>,
        function_ids: Vec<HostFunctionId>,
    ) -> Self {
        Self {
            prototype,
            prototype_id,
            functions,
            function_ids,
        }
    }

    pub(super) fn stage_function(
        &mut self,
        context: &mut Context,
        function: HostFunction,
    ) -> Result<Value> {
        let retained = context.create_retained_host_function_value(function)?;
        let value = context.resolve_retained_value(&retained)?;
        let Value::HostFunction(id) = value else {
            return Err(Error::runtime(
                "host class member allocation returned a non-function",
            ));
        };
        self.function_ids.push(id);
        self.functions.push(retained);
        Ok(Value::HostFunction(id))
    }

    pub(super) fn rollback(self, context: &mut Context) -> Result<()> {
        let Self {
            prototype,
            prototype_id,
            functions,
            function_ids,
        } = self;
        drop(functions);
        drop(prototype);
        context.rollback_host_class_graph(prototype_id, &function_ids)
    }
}
