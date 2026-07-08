use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

const ARRAY_FLATTEN_INDEX_LIMIT_ERROR: &str = "array flatten index exceeded supported range";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FlattenDepth {
    Finite(usize),
    Infinity,
}

impl Context {
    pub(in crate::runtime::native) fn eval_array_flat(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_flat(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_flat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let depth = Self::flat_depth_arg(args)?;
        Self::ensure_array_like_object(this_value)?;
        let source_length = self.array_like_length(this_value)?;
        let result = self.create_array_callback_result(0)?;
        let mut target_index = 0;
        self.flatten_array_like_into(this_value, source_length, &result, &mut target_index, depth)?;
        self.finish_flatten_result_length(&result, target_index)?;
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_flat_map(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_flat_map(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_flat_map(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        Self::ensure_array_like_object(this_value)?;
        let source_length = self.array_like_length(this_value)?;
        let result = self.create_array_callback_result(0)?;
        let mut target_index = 0;
        for source_index in 0..source_length {
            self.step()?;
            if !self.has_array_like_index(this_value, source_index)? {
                continue;
            }
            let value = self.get_array_like_index(this_value, source_index)?;
            let mapped = self.call_array_callback(
                callback,
                callback_this.clone(),
                &value,
                source_index,
                this_value,
            )?;
            self.flatten_value_into(mapped, &result, &mut target_index, FlattenDepth::Finite(1))?;
        }
        self.finish_flatten_result_length(&result, target_index)?;
        Ok(result)
    }

    fn flatten_array_like_into(
        &mut self,
        source: &Value,
        source_length: usize,
        result: &Value,
        target_index: &mut usize,
        depth: FlattenDepth,
    ) -> Result<()> {
        for source_index in 0..source_length {
            self.step()?;
            if !self.has_array_like_index(source, source_index)? {
                continue;
            }
            let value = self.get_array_like_index(source, source_index)?;
            self.flatten_value_into(value, result, target_index, depth)?;
        }
        Ok(())
    }

    fn flatten_value_into(
        &mut self,
        value: Value,
        result: &Value,
        target_index: &mut usize,
        depth: FlattenDepth,
    ) -> Result<()> {
        if let Some(next_depth) = depth.descend()
            && self.is_flattenable_array(&value)?
        {
            let nested_length = self.array_like_length(&value)?;
            return self.flatten_array_like_into(
                &value,
                nested_length,
                result,
                target_index,
                next_depth,
            );
        }
        self.append_flattened_value(result, target_index, value)
    }

    fn append_flattened_value(
        &mut self,
        result: &Value,
        target_index: &mut usize,
        value: Value,
    ) -> Result<()> {
        self.set_array_like_index(result, *target_index, value)?;
        *target_index = target_index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_FLATTEN_INDEX_LIMIT_ERROR))?;
        Ok(())
    }

    fn is_flattenable_array(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(self.objects.array_len_if_array(*id)?.is_some())
    }

    fn finish_flatten_result_length(&mut self, result: &Value, length: usize) -> Result<()> {
        if let Value::Object(id) = result
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return Ok(());
        }
        self.set_array_like_length(result, length)
    }

    fn flat_depth_arg(args: &[Value]) -> Result<FlattenDepth> {
        let Some(value) = args.first() else {
            return Ok(FlattenDepth::Finite(1));
        };
        if matches!(value, Value::Undefined) {
            return Ok(FlattenDepth::Finite(1));
        }
        Self::value_to_flatten_depth(value)
    }

    fn value_to_flatten_depth(value: &Value) -> Result<FlattenDepth> {
        let number = Self::value_to_number(value);
        if number.is_nan() || number <= 0.0 {
            return Ok(FlattenDepth::Finite(0));
        }
        if !number.is_finite() {
            return Ok(FlattenDepth::Infinity);
        }
        Self::finite_flatten_depth(number)
    }

    fn finite_flatten_depth(value: f64) -> Result<FlattenDepth> {
        format!("{:.0}", value.floor())
            .parse::<usize>()
            .map(FlattenDepth::Finite)
            .map_err(|_| Error::limit("array flatten depth exceeded supported range"))
    }
}

impl FlattenDepth {
    const fn descend(self) -> Option<Self> {
        match self {
            Self::Finite(0) => None,
            Self::Finite(value) => Some(Self::Finite(value.saturating_sub(1))),
            Self::Infinity => Some(Self::Infinity),
        }
    }
}
