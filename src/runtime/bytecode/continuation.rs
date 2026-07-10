use crate::{
    bytecode::BytecodeBlock,
    error::{Error, Result},
    runtime::{Context, VmStorageKind, activation::ActivationFrame},
    value::{FunctionId, Value},
};

use super::control_continuation::BytecodeControlRecord;
use super::state::BytecodeState;

/// VM-owned continuation state for one active bytecode block.
///
/// While the frame is executing, its state is temporarily checked out by the
/// synchronous driver and the existing transient-root scope owns its operand
/// values. A future suspended outcome can leave the state parked here without
/// changing the root or storage ownership model.
#[derive(Debug)]
pub(in crate::runtime) struct BytecodeContinuationFrame {
    program: BytecodeContinuationProgram,
    parked_state: Option<BytecodeState>,
    control_stack: Vec<Option<BytecodeControlRecord>>,
}

#[derive(Debug)]
enum BytecodeContinuationProgram {
    Function(FunctionId),
    Block { _block: BytecodeBlock },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BytecodeContinuationHandle {
    activation_index: usize,
    owns_activation: bool,
}

impl BytecodeContinuationFrame {
    pub(in crate::runtime) const fn function(function: FunctionId) -> Self {
        Self {
            program: BytecodeContinuationProgram::Function(function),
            parked_state: None,
            control_stack: Vec::new(),
        }
    }

    pub(in crate::runtime) const fn block(block: BytecodeBlock) -> Self {
        Self {
            program: BytecodeContinuationProgram::Block { _block: block },
            parked_state: None,
            control_stack: Vec::new(),
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.parked_state
            .iter()
            .flat_map(BytecodeState::root_values)
            .chain(
                self.control_stack
                    .iter()
                    .flatten()
                    .flat_map(BytecodeControlRecord::root_values),
            )
    }

    pub(in crate::runtime) const fn function_id(&self) -> Option<FunctionId> {
        match self.program {
            BytecodeContinuationProgram::Function(function) => Some(function),
            BytecodeContinuationProgram::Block { .. } => None,
        }
    }

    pub(in crate::runtime) const fn is_settled(&self) -> bool {
        self.parked_state.is_none() && self.control_stack.is_empty()
    }

    const fn is_running(&self) -> bool {
        self.parked_state.is_none()
    }

    pub(in crate::runtime) const fn control_count(&self) -> usize {
        self.control_stack.len()
    }

    pub(super) fn push_control(&mut self, record: BytecodeControlRecord) -> usize {
        let index = self.control_stack.len();
        self.control_stack.push(Some(record));
        index
    }

    pub(super) fn checkout_control(&mut self, index: usize) -> Result<BytecodeControlRecord> {
        let expected = self
            .control_stack
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control stack is empty"))?;
        if index != expected {
            return Err(Error::runtime("structured control checkout mismatch"));
        }
        self.control_stack
            .get_mut(index)
            .and_then(Option::take)
            .ok_or_else(|| Error::runtime("structured control record is already running"))
    }

    pub(super) fn finish_control(&mut self, index: usize) -> Result<()> {
        let expected = self
            .control_stack
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control stack is empty"))?;
        if index != expected {
            return Err(Error::runtime("structured control unwind mismatch"));
        }
        if self.control_stack.last().is_some_and(Option::is_some) {
            return Err(Error::runtime(
                "parked structured control cannot finish synchronously",
            ));
        }
        let _slot = self
            .control_stack
            .pop()
            .ok_or_else(|| Error::runtime("structured control slot disappeared"))?;
        Ok(())
    }
}

impl Context {
    pub(super) fn push_bytecode_continuation(
        &mut self,
        block: &BytecodeBlock,
    ) -> Result<BytecodeContinuationHandle> {
        let attaches_to_current = self
            .activation_frames
            .last()
            .is_some_and(|activation| activation.continuation().is_none());
        if !attaches_to_current {
            self.storage_ledger
                .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        }
        let continuation = BytecodeContinuationFrame::block(block.clone());
        if attaches_to_current {
            let index = self
                .activation_frames
                .len()
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("activation stack is empty"))?;
            let activation = self
                .activation_frames
                .get_mut(index)
                .ok_or_else(|| Error::runtime("activation frame disappeared"))?;
            *activation.continuation_mut() = Some(continuation);
            return Ok(BytecodeContinuationHandle {
                activation_index: index,
                owns_activation: false,
            });
        }
        let index = self.activation_frames.len();
        self.activation_frames
            .push(ActivationFrame::bytecode(continuation));
        Ok(BytecodeContinuationHandle {
            activation_index: index,
            owns_activation: true,
        })
    }

    pub(super) fn pop_bytecode_continuation(
        &mut self,
        handle: BytecodeContinuationHandle,
    ) -> Result<()> {
        let expected = self
            .activation_frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("bytecode continuation stack is empty"))?;
        if handle.activation_index != expected {
            return Err(Error::runtime("bytecode continuation unwind mismatch"));
        }
        let activation = self
            .activation_frames
            .get(handle.activation_index)
            .ok_or_else(|| Error::runtime("bytecode continuation frame disappeared"))?;
        let continuation = activation
            .continuation()
            .ok_or_else(|| Error::runtime("bytecode continuation state disappeared"))?;
        if !continuation.is_settled() {
            return Err(Error::runtime(
                "parked bytecode continuation cannot be synchronously removed",
            ));
        }
        if handle.owns_activation {
            if !self
                .activation_frames
                .last()
                .is_some_and(ActivationFrame::is_bytecode)
            {
                return Err(Error::runtime("bytecode continuation owner mismatch"));
            }
            let mut activation = self
                .activation_frames
                .pop()
                .ok_or_else(|| Error::runtime("bytecode continuation frame disappeared"))?;
            self.storage_ledger
                .release_count(VmStorageKind::ExecutionFrame, 1)?;
            let _continuation = activation
                .continuation_mut()
                .take()
                .ok_or_else(|| Error::runtime("bytecode continuation state disappeared"))?;
        } else {
            let _continuation = self
                .activation_frames
                .get_mut(handle.activation_index)
                .and_then(|activation| activation.continuation_mut().take())
                .ok_or_else(|| Error::runtime("bytecode continuation state disappeared"))?;
        }
        Ok(())
    }

    pub(super) fn ensure_running_function_continuation(&self, function: FunctionId) -> Result<()> {
        let continuation = self
            .activation_frames
            .last()
            .and_then(ActivationFrame::continuation)
            .ok_or_else(|| Error::runtime("function bytecode continuation disappeared"))?;
        if continuation.function_id() != Some(function) || !continuation.is_running() {
            return Err(Error::runtime("function bytecode continuation mismatch"));
        }
        Ok(())
    }
}
