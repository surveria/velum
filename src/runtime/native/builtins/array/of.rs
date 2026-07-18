use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, roots::VmRootKind},
    value::Value,
};

impl Context {
    pub(in crate::runtime::native) fn eval_array_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let values = args.as_slice();
        let result = self.array_from_result(this_value, Some(values.len()))?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&result))?;
        for (index, value) in values.iter().cloned().enumerate() {
            self.array_from_create_data_property(&result, index, value)?;
        }
        self.set_array_like_length(&result, values.len())?;
        Ok(result)
    }
}
