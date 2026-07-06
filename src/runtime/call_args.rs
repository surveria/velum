use crate::value::Value;

#[derive(Debug, Clone, Copy)]
pub enum RuntimeCallArgs<'a> {
    Values(&'a [Value]),
}

impl<'a> RuntimeCallArgs<'a> {
    pub const fn values(args: &'a [Value]) -> Self {
        Self::Values(args)
    }

    pub fn evaluate(self) -> Vec<Value> {
        match self {
            Self::Values(args) => args.to_vec(),
        }
    }

    pub fn unary_value(self) -> Option<Value> {
        match self {
            Self::Values(args) => args.first().cloned(),
        }
    }

    pub fn binary_values(self) -> (Option<Value>, Option<Value>) {
        match self {
            Self::Values(args) => (args.first().cloned(), args.get(1).cloned()),
        }
    }

    pub const fn discard(self) {
        match self {
            Self::Values(_) => {}
        }
    }
}
