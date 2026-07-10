use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

const INDEX_NOT_FOUND: f64 = -1.0;

impl Context {
    pub(in crate::runtime::native) fn eval_array_find_last(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_find_last(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_find_last(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        for index in (0..length).rev() {
            self.step()?;
            let value = self.get_array_like_index(this_value, index)?;
            let result = self.call_array_callback(
                callback,
                callback_this.clone(),
                &value,
                index,
                this_value,
            )?;
            if result.is_truthy() {
                return Ok(value);
            }
        }
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_array_find_last_index(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_find_last_index(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_find_last_index(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        for index in (0..length).rev() {
            self.step()?;
            let value = self.get_array_like_index(this_value, index)?;
            let result = self.call_array_callback(
                callback,
                callback_this.clone(),
                &value,
                index,
                this_value,
            )?;
            if result.is_truthy() {
                return Self::array_like_index_value(index);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }
}
