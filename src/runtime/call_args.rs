use crate::value::Value;

#[derive(Debug, Clone, Copy)]
pub enum RuntimeCallArgs<'a> {
    Values(&'a [Value]),
}

impl<'a> RuntimeCallArgs<'a> {
    pub const fn values(args: &'a [Value]) -> Self {
        Self::Values(args)
    }

    pub const fn as_slice(self) -> &'a [Value] {
        match self {
            Self::Values(args) => args,
        }
    }

    pub fn evaluate(self) -> Vec<Value> {
        self.as_slice().to_vec()
    }

    pub const fn discard(self) {
        match self {
            Self::Values(_) => {}
        }
    }
}
