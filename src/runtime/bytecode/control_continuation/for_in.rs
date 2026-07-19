#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    value::Value,
};

use super::{BytecodeControlRecord, BytecodeLoopPhase, BytecodeState};

impl BytecodeControlRecord {
    pub(in crate::runtime::bytecode) fn for_in(keys: Vec<String>, source: Option<Value>) -> Self {
        Self::ForIn {
            phase: BytecodeLoopPhase::Initialize,
            keys: keys.into_iter(),
            source,
            body_state: BytecodeState::new(),
            last: Value::Undefined,
        }
    }

    pub(in crate::runtime::bytecode) fn for_in_state_mut(
        &mut self,
    ) -> Result<(
        &mut BytecodeLoopPhase,
        &mut alloc::vec::IntoIter<String>,
        &mut Value,
    )> {
        let Self::ForIn {
            phase, keys, last, ..
        } = self
        else {
            return Err(Error::runtime("structured for-in record mismatch"));
        };
        Ok((phase, keys, last))
    }

    pub(in crate::runtime::bytecode) fn for_in_source(&self) -> Result<Option<&Value>> {
        let Self::ForIn { source, .. } = self else {
            return Err(Error::runtime("structured for-in record mismatch"));
        };
        Ok(source.as_ref())
    }
}
