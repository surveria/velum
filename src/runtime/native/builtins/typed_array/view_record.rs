use crate::{
    error::Result,
    runtime::{Context, object::TypedArrayView},
    value::Value,
};

#[derive(Debug, Clone)]
pub(super) struct TypedArrayViewRecord {
    pub(super) view: TypedArrayView,
    pub(super) length: usize,
}

impl TypedArrayViewRecord {
    pub(super) fn read(&self, index: usize) -> Result<Option<Value>> {
        self.view.read(index)
    }

    pub(super) fn value(&self, index: usize) -> Result<Value> {
        self.read(index)
            .map(|value| value.unwrap_or(Value::Undefined))
    }
}

impl Context {
    pub(super) fn typed_array_view_record(
        &self,
        this_value: &Value,
    ) -> Result<TypedArrayViewRecord> {
        let (_, view) = self.typed_array_receiver(this_value)?;
        let length = view.length();
        Ok(TypedArrayViewRecord { view, length })
    }
}
