#[cfg(not(feature = "std"))]
use crate::prelude::*;

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

    pub fn to_owned_values(self) -> Vec<Value> {
        self.as_slice().to_vec()
    }
}
