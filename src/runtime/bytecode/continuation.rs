use std::cmp::Ordering;

use crate::{
    bytecode::BytecodeBlock,
    error::{Error, Result},
    runtime::{Context, VmStorageKind, activation::ActivationFrame, control::Completion},
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
    parked_state: Option<Box<BytecodeState>>,
    control_stack: Vec<Option<BytecodeControlRecord>>,
    control_cursor: usize,
    resumed_child: Option<Box<(BytecodeBlock, Completion)>>,
}

const fn completion_value(completion: &Completion) -> Option<&Value> {
    match completion {
        Completion::Normal(value)
        | Completion::Throw(value)
        | Completion::Return(value)
        | Completion::ReturnDirect(value)
        | Completion::Break { value, .. }
        | Completion::Continue { value, .. }
        | Completion::Yielded(value)
        | Completion::YieldedIteratorResult(value) => Some(value),
        Completion::Suspended(_) | Completion::GeneratorStart => None,
    }
}

#[derive(Debug)]
enum BytecodeContinuationProgram {
    Function(FunctionId),
    Block { block: BytecodeBlock },
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
            control_cursor: 0,
            resumed_child: None,
        }
    }

    pub(in crate::runtime) const fn block(block: BytecodeBlock) -> Self {
        Self {
            program: BytecodeContinuationProgram::Block { block },
            parked_state: None,
            control_stack: Vec::new(),
            control_cursor: 0,
            resumed_child: None,
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.parked_state
            .iter()
            .flat_map(|state| state.root_values())
            .chain(
                self.control_stack
                    .iter()
                    .flatten()
                    .flat_map(BytecodeControlRecord::root_values),
            )
            .chain(
                self.resumed_child
                    .iter()
                    .filter_map(|resumed| completion_value(&resumed.1)),
            )
    }

    pub(in crate::runtime) const fn function_id(&self) -> Option<FunctionId> {
        match self.program {
            BytecodeContinuationProgram::Function(function) => Some(function),
            BytecodeContinuationProgram::Block { .. } => None,
        }
    }

    pub(in crate::runtime) const fn is_settled(&self) -> bool {
        self.parked_state.is_none()
            && self.control_stack.is_empty()
            && self.control_cursor == 0
            && self.resumed_child.is_none()
    }

    pub(in crate::runtime) fn has_yield_delegate(&self) -> bool {
        self.parked_state
            .as_ref()
            .is_some_and(|state| state.has_yield_delegate())
    }

    const fn is_running(&self) -> bool {
        self.parked_state.is_none()
    }

    pub(in crate::runtime) const fn control_count(&self) -> usize {
        self.control_stack.len()
    }

    pub(super) const fn resumes_control(&self) -> bool {
        self.control_cursor < self.control_stack.len()
    }

    pub(super) fn enter_control(&mut self, record: BytecodeControlRecord) -> Result<usize> {
        let index = self.control_cursor;
        match index.cmp(&self.control_stack.len()) {
            Ordering::Less => {
                if !self.control_stack.get(index).is_some_and(Option::is_some) {
                    return Err(Error::runtime(
                        "structured control resume record is already running",
                    ));
                }
            }
            Ordering::Equal => self.control_stack.push(Some(record)),
            Ordering::Greater => {
                return Err(Error::runtime("structured control cursor overflowed"));
            }
        }
        self.control_cursor = self
            .control_cursor
            .checked_add(1)
            .ok_or_else(|| Error::limit("structured control cursor overflowed"))?;
        Ok(index)
    }

    pub(super) fn checkout_control(&mut self, index: usize) -> Result<BytecodeControlRecord> {
        let expected = self
            .control_cursor
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control cursor is empty"))?;
        if index != expected {
            return Err(Error::runtime("structured control checkout mismatch"));
        }
        self.control_stack
            .get_mut(index)
            .and_then(Option::take)
            .ok_or_else(|| Error::runtime("structured control record is already running"))
    }

    pub(super) fn park_control(
        &mut self,
        index: usize,
        record: BytecodeControlRecord,
    ) -> Result<()> {
        let slot = self
            .control_stack
            .get_mut(index)
            .ok_or_else(|| Error::runtime("structured control slot disappeared"))?;
        if slot.is_some() {
            return Err(Error::runtime(
                "structured control record is already parked",
            ));
        }
        *slot = Some(record);
        let expected = self
            .control_cursor
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control cursor is empty"))?;
        if index != expected {
            return Err(Error::runtime("structured control park mismatch"));
        }
        self.control_cursor = expected;
        Ok(())
    }

    pub(in crate::runtime) fn park_state(&mut self, state: BytecodeState) -> Result<()> {
        if self.parked_state.is_some() {
            return Err(Error::runtime("bytecode state is already parked"));
        }
        self.parked_state = Some(Box::new(state));
        Ok(())
    }

    pub(in crate::runtime) fn checkout_state(&mut self) -> Result<BytecodeState> {
        if self.control_cursor != 0 {
            return Err(Error::runtime(
                "structured control cursor was not parked before resume",
            ));
        }
        self.parked_state
            .take()
            .map(|state| *state)
            .ok_or_else(|| Error::runtime("bytecode state is not parked"))
    }

    pub(in crate::runtime) fn resume_suspension(&mut self, completion: Completion) -> Result<()> {
        if self.parked_state.as_ref().is_some_and(|state| {
            state.is_awaiting() || state.is_generator_starting() || state.is_yielding()
        }) {
            return self
                .parked_state
                .as_mut()
                .ok_or_else(|| Error::runtime("parked bytecode state disappeared"))?
                .resume_suspension(completion);
        }
        for record in self.control_stack.iter_mut().rev().flatten() {
            if record.resume_suspension(completion.clone())? {
                return Ok(());
            }
        }
        Err(Error::runtime(
            "suspended bytecode continuation has no resumable state",
        ))
    }

    pub(in crate::runtime) fn program_block(&self) -> Option<BytecodeBlock> {
        match &self.program {
            BytecodeContinuationProgram::Function(_) => None,
            BytecodeContinuationProgram::Block { block } => Some(block.clone()),
        }
    }

    pub(in crate::runtime) fn store_resumed_child(
        &mut self,
        block: BytecodeBlock,
        completion: Completion,
    ) -> Result<()> {
        if self.resumed_child.is_some() {
            return Err(Error::runtime("resumed bytecode child is already stored"));
        }
        self.resumed_child = Some(Box::new((block, completion)));
        Ok(())
    }

    pub(in crate::runtime) fn has_resumed_child(&self, block: &BytecodeBlock) -> bool {
        self.resumed_child
            .as_ref()
            .is_some_and(|resumed| &resumed.0 == block)
    }

    pub(in crate::runtime) fn take_resumed_child(
        &mut self,
        block: &BytecodeBlock,
    ) -> Result<Option<Completion>> {
        if !self.has_resumed_child(block) {
            return Ok(None);
        }
        let resumed = self
            .resumed_child
            .take()
            .ok_or_else(|| Error::runtime("resumed bytecode child disappeared"))?;
        let (_, completion) = *resumed;
        Ok(Some(completion))
    }

    pub(super) fn finish_control(&mut self, index: usize) -> Result<()> {
        let expected = self
            .control_cursor
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
        self.control_cursor = expected;
        Ok(())
    }
}

impl Context {
    pub(super) fn take_resumed_bytecode_child(
        &mut self,
        block: &BytecodeBlock,
    ) -> Result<Option<Completion>> {
        let Some(continuation) = self
            .activation_frames
            .last_mut()
            .map(ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
        else {
            return Ok(None);
        };
        continuation.take_resumed_child(block)
    }

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
        let private_environment = self.current_private_environment();
        let with_environments = self.current_with_environments().to_vec();
        self.activation_frames
            .push(ActivationFrame::bytecode(continuation, with_environments));
        self.set_current_private_environment(private_environment)?;
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
            return Err(Error::runtime(format!(
                "bytecode continuation unwind mismatch: expected {expected}, actual {}",
                handle.activation_index
            )));
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

    pub(super) fn park_bytecode_state_at(
        &mut self,
        activation_index: usize,
        state: BytecodeState,
    ) -> Result<()> {
        self.activation_frames
            .get_mut(activation_index)
            .map(ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::runtime("bytecode continuation disappeared"))?
            .park_state(state)
    }

    pub(super) fn park_bytecode_continuation_state(
        &mut self,
        handle: BytecodeContinuationHandle,
        state: BytecodeState,
    ) -> Result<()> {
        self.park_bytecode_state_at(handle.activation_index, state)
    }
}
