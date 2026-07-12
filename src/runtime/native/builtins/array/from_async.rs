use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

impl Context {
    pub(in crate::runtime::native) fn eval_array_from_async(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.start_array_from_async(args.as_slice(), this_value)
    }
}
