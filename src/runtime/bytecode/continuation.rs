use crate::{
    bytecode::BytecodeBlock,
    error::{Error, Result},
    runtime::{Context, VmStorageKind, activation::ActivationFrame},
    value::Value,
};

use super::state::BytecodeState;

/// VM-owned continuation state for one active bytecode block.
///
/// While the frame is executing, its state is temporarily checked out by the
/// synchronous driver and the existing transient-root scope owns its operand
/// values. A future suspended outcome can leave the state parked here without
/// changing the root or storage ownership model.
#[derive(Debug)]
pub(in crate::runtime) struct BytecodeContinuationFrame {
    block: BytecodeBlock,
    state: Option<BytecodeState>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BytecodeContinuationHandle {
    activation_index: usize,
    owns_activation: bool,
}

impl BytecodeContinuationFrame {
    pub(in crate::runtime) const fn new(block: BytecodeBlock, state: BytecodeState) -> Self {
        Self {
            block,
            state: Some(state),
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.state.iter().flat_map(BytecodeState::root_values)
    }

    fn take(&mut self) -> Result<(BytecodeBlock, BytecodeState)> {
        let state = self
            .state
            .take()
            .ok_or_else(|| Error::runtime("bytecode continuation state is already running"))?;
        Ok((self.block.clone(), state))
    }

    fn restore(&mut self, state: BytecodeState) -> Result<()> {
        if self.state.is_some() {
            return Err(Error::runtime(
                "bytecode continuation state was restored twice",
            ));
        }
        self.state = Some(state);
        Ok(())
    }

    fn into_state(self) -> Result<BytecodeState> {
        self.state
            .ok_or_else(|| Error::runtime("running bytecode continuation cannot be removed"))
    }
}

impl Context {
    pub(super) fn push_bytecode_continuation(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<BytecodeContinuationHandle> {
        let attaches_to_current = self
            .activation_frames
            .last()
            .is_some_and(|activation| activation.continuation().is_none());
        if !attaches_to_current {
            self.storage_ledger
                .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        }
        state.reset();
        let owned_state = std::mem::replace(state, BytecodeState::new());
        let continuation = BytecodeContinuationFrame::new(block.clone(), owned_state);
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

    pub(super) fn take_bytecode_continuation(
        &mut self,
        handle: BytecodeContinuationHandle,
    ) -> Result<(BytecodeBlock, BytecodeState)> {
        self.activation_frames
            .get_mut(handle.activation_index)
            .and_then(|activation| activation.continuation_mut().as_mut())
            .ok_or_else(|| Error::runtime("bytecode continuation frame disappeared"))?
            .take()
    }

    pub(super) fn restore_bytecode_continuation(
        &mut self,
        handle: BytecodeContinuationHandle,
        state: BytecodeState,
    ) -> Result<()> {
        self.activation_frames
            .get_mut(handle.activation_index)
            .and_then(|activation| activation.continuation_mut().as_mut())
            .ok_or_else(|| Error::runtime("bytecode continuation frame disappeared"))?
            .restore(state)
    }

    pub(super) fn pop_bytecode_continuation(
        &mut self,
        handle: BytecodeContinuationHandle,
    ) -> Result<BytecodeState> {
        let expected = self
            .activation_frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("bytecode continuation stack is empty"))?;
        if handle.activation_index != expected {
            return Err(Error::runtime("bytecode continuation unwind mismatch"));
        }
        let continuation = if handle.owns_activation {
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
            activation
                .continuation_mut()
                .take()
                .ok_or_else(|| Error::runtime("bytecode continuation state disappeared"))?
        } else {
            self.activation_frames
                .get_mut(handle.activation_index)
                .and_then(|activation| activation.continuation_mut().take())
                .ok_or_else(|| Error::runtime("bytecode continuation state disappeared"))?
        };
        continuation.into_state()
    }
}
