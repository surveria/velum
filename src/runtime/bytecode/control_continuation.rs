use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind, abstract_operations::IteratorSource, activation::ActivationFrame,
        control::Completion,
    },
    value::Value,
};

use super::state::BytecodeState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BytecodeControlStateSlot {
    Condition,
    Body,
    Update,
    Catch,
    Finally,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BytecodeLoopKind {
    While,
    DoWhile,
    For,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BytecodeLoopPhase {
    Initialize,
    Condition,
    Body,
    Update,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BytecodeTryPhase {
    Body,
    Catch,
    Finally,
}

/// Durable state for one structured-control construct.
///
/// The synchronous driver checks the record out while it is running. Its
/// values stay in transient-root scopes during nested execution. A suspended
/// driver can instead park the complete record in its continuation slot.
#[derive(Debug)]
pub(super) enum BytecodeControlRecord {
    Loop {
        kind: BytecodeLoopKind,
        phase: BytecodeLoopPhase,
        condition_state: BytecodeState,
        body_state: BytecodeState,
        update_state: BytecodeState,
        last: Value,
    },
    ForIn {
        phase: BytecodeLoopPhase,
        keys: std::vec::IntoIter<String>,
        body_state: BytecodeState,
        last: Value,
    },
    ForOf {
        phase: BytecodeLoopPhase,
        source: Option<IteratorSource>,
        body_state: BytecodeState,
        last: Value,
    },
    Switch {
        body_state: BytecodeState,
        next_case: usize,
        last: Value,
    },
    Try {
        phase: BytecodeTryPhase,
        body_state: BytecodeState,
        catch_state: BytecodeState,
        finally_state: BytecodeState,
        pending: Option<Completion>,
    },
}

impl BytecodeControlRecord {
    pub(super) fn resume_await(&mut self, completion: Completion) -> Result<bool> {
        let state = match self {
            Self::Loop {
                phase,
                condition_state,
                body_state,
                update_state,
                ..
            } => match phase {
                BytecodeLoopPhase::Initialize | BytecodeLoopPhase::Condition => condition_state,
                BytecodeLoopPhase::Body => body_state,
                BytecodeLoopPhase::Update => update_state,
            },
            Self::ForIn { body_state, .. }
            | Self::ForOf { body_state, .. }
            | Self::Switch { body_state, .. } => body_state,
            Self::Try {
                phase,
                body_state,
                catch_state,
                finally_state,
                ..
            } => match phase {
                BytecodeTryPhase::Body => body_state,
                BytecodeTryPhase::Catch => catch_state,
                BytecodeTryPhase::Finally => finally_state,
            },
        };
        if !state.is_suspended() {
            return Ok(false);
        }
        state.resume_await(completion)?;
        Ok(true)
    }

    pub(super) const fn loop_record(kind: BytecodeLoopKind) -> Self {
        Self::Loop {
            kind,
            phase: BytecodeLoopPhase::Initialize,
            condition_state: BytecodeState::new(),
            body_state: BytecodeState::new(),
            update_state: BytecodeState::new(),
            last: Value::Undefined,
        }
    }

    pub(super) const fn switch(next_case: usize) -> Self {
        Self::Switch {
            body_state: BytecodeState::new(),
            next_case,
            last: Value::Undefined,
        }
    }

    pub(super) fn for_in(keys: Vec<String>) -> Self {
        Self::ForIn {
            phase: BytecodeLoopPhase::Initialize,
            keys: keys.into_iter(),
            body_state: BytecodeState::new(),
            last: Value::Undefined,
        }
    }

    pub(super) const fn for_of(source: IteratorSource) -> Self {
        Self::ForOf {
            phase: BytecodeLoopPhase::Initialize,
            source: Some(source),
            body_state: BytecodeState::new(),
            last: Value::Undefined,
        }
    }

    pub(super) const fn try_record() -> Self {
        Self::Try {
            phase: BytecodeTryPhase::Body,
            body_state: BytecodeState::new(),
            catch_state: BytecodeState::new(),
            finally_state: BytecodeState::new(),
            pending: None,
        }
    }

    pub(super) fn root_values(&self) -> impl Iterator<Item = &Value> {
        let mut roots = Vec::new();
        match self {
            Self::Loop {
                condition_state,
                body_state,
                update_state,
                last,
                ..
            } => {
                roots.extend(condition_state.root_values());
                roots.extend(body_state.root_values());
                roots.extend(update_state.root_values());
                roots.push(last);
            }
            Self::Switch {
                body_state, last, ..
            }
            | Self::ForIn {
                body_state, last, ..
            } => {
                roots.extend(body_state.root_values());
                roots.push(last);
            }
            Self::ForOf {
                source,
                body_state,
                last,
                ..
            } => {
                if let Some(source) = source {
                    roots.extend(source.root_values());
                }
                roots.extend(body_state.root_values());
                roots.push(last);
            }
            Self::Try {
                body_state,
                catch_state,
                finally_state,
                pending,
                ..
            } => {
                roots.extend(body_state.root_values());
                roots.extend(catch_state.root_values());
                roots.extend(finally_state.root_values());
                if let Some(value) = pending.as_ref().and_then(completion_value) {
                    roots.push(value);
                }
            }
        }
        roots.into_iter()
    }

    pub(super) fn loop_state_mut(
        &mut self,
        expected: BytecodeLoopKind,
    ) -> Result<(&mut BytecodeLoopPhase, &mut Value)> {
        let Self::Loop {
            kind, phase, last, ..
        } = self
        else {
            return Err(Error::runtime("structured loop record mismatch"));
        };
        if *kind != expected {
            return Err(Error::runtime("structured loop kind mismatch"));
        }
        Ok((phase, last))
    }

    pub(super) fn try_state_mut(
        &mut self,
    ) -> Result<(&mut BytecodeTryPhase, &mut Option<Completion>)> {
        let Self::Try { phase, pending, .. } = self else {
            return Err(Error::runtime("structured try record mismatch"));
        };
        Ok((phase, pending))
    }

    pub(super) fn switch_state_mut(&mut self) -> Result<(&mut usize, &mut Value)> {
        let Self::Switch {
            next_case, last, ..
        } = self
        else {
            return Err(Error::runtime("structured switch record mismatch"));
        };
        Ok((next_case, last))
    }

    pub(super) fn for_in_state_mut(
        &mut self,
    ) -> Result<(
        &mut BytecodeLoopPhase,
        &mut std::vec::IntoIter<String>,
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

    pub(super) fn for_of_state_mut(&mut self) -> Result<(&mut BytecodeLoopPhase, &mut Value)> {
        let Self::ForOf { phase, last, .. } = self else {
            return Err(Error::runtime("structured for-of record mismatch"));
        };
        Ok((phase, last))
    }

    fn iterator_source_mut(&mut self) -> Result<&mut IteratorSource> {
        let Self::ForOf { source, .. } = self else {
            return Err(Error::runtime("structured iterator record mismatch"));
        };
        source
            .as_mut()
            .ok_or_else(|| Error::runtime("structured iterator source disappeared"))
    }

    fn transient_root_values(&self) -> impl Iterator<Item = &Value> {
        let roots = match self {
            Self::Loop { last, .. } | Self::ForIn { last, .. } | Self::Switch { last, .. } => {
                [Some(last), None, None]
            }
            Self::ForOf { source, last, .. } => {
                let [first, second] = source
                    .as_ref()
                    .map_or([None, None], IteratorSource::root_value_slots);
                [Some(last), first, second]
            }
            Self::Try { pending, .. } => [pending.as_ref().and_then(completion_value), None, None],
        };
        roots.into_iter().flatten()
    }

    fn has_traceable_transient_roots(&self) -> bool {
        match self {
            Self::Loop { last, .. } | Self::ForIn { last, .. } | Self::Switch { last, .. } => {
                crate::runtime::transient_roots::is_traceable(last)
            }
            Self::ForOf { source, last, .. } => {
                crate::runtime::transient_roots::is_traceable(last)
                    || source.as_ref().is_some_and(|source| {
                        let [first, second] = source.root_value_slots();
                        first.is_some_and(crate::runtime::transient_roots::is_traceable)
                            || second.is_some_and(crate::runtime::transient_roots::is_traceable)
                    })
            }
            Self::Try { pending, .. } => pending
                .as_ref()
                .and_then(completion_value)
                .is_some_and(crate::runtime::transient_roots::is_traceable),
        }
    }

    fn state_mut(&mut self, slot: BytecodeControlStateSlot) -> Result<&mut BytecodeState> {
        match (self, slot) {
            (
                Self::Loop {
                    condition_state, ..
                },
                BytecodeControlStateSlot::Condition,
            ) => Ok(condition_state),
            (
                Self::Loop { body_state, .. }
                | Self::ForIn { body_state, .. }
                | Self::ForOf { body_state, .. }
                | Self::Switch { body_state, .. }
                | Self::Try { body_state, .. },
                BytecodeControlStateSlot::Body,
            ) => Ok(body_state),
            (Self::Loop { update_state, .. }, BytecodeControlStateSlot::Update) => Ok(update_state),
            (Self::Try { catch_state, .. }, BytecodeControlStateSlot::Catch) => Ok(catch_state),
            (Self::Try { finally_state, .. }, BytecodeControlStateSlot::Finally) => {
                Ok(finally_state)
            }
            _ => Err(Error::runtime("structured control state slot mismatch")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BytecodeControlHandle {
    activation_index: usize,
    control_index: usize,
}

impl Context {
    pub(super) fn resumes_bytecode_control(&self) -> bool {
        self.activation_frames
            .last()
            .and_then(ActivationFrame::continuation)
            .is_some_and(super::BytecodeContinuationFrame::resumes_control)
    }

    pub(super) fn push_bytecode_control(
        &mut self,
        record: BytecodeControlRecord,
    ) -> Result<BytecodeControlHandle> {
        let activation_index = self
            .activation_frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control has no activation owner"))?;
        if self
            .activation_frames
            .get(activation_index)
            .and_then(ActivationFrame::continuation)
            .is_none()
        {
            return Err(Error::runtime(
                "structured control has no continuation owner",
            ));
        }
        let resumes = self
            .activation_frames
            .get(activation_index)
            .and_then(ActivationFrame::continuation)
            .is_some_and(super::BytecodeContinuationFrame::resumes_control);
        if !resumes {
            self.storage_ledger
                .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        }
        let continuation = self
            .activation_frames
            .get_mut(activation_index)
            .map(ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::runtime("structured control continuation disappeared"))?;
        let control_index = match continuation.enter_control(record) {
            Ok(index) => index,
            Err(error) => {
                if !resumes {
                    self.storage_ledger
                        .release_count(VmStorageKind::ExecutionFrame, 1)?;
                }
                return Err(error);
            }
        };
        Ok(BytecodeControlHandle {
            activation_index,
            control_index,
        })
    }

    pub(super) fn checkout_bytecode_control(
        &mut self,
        handle: BytecodeControlHandle,
    ) -> Result<BytecodeControlRecord> {
        self.bytecode_control_continuation_mut(handle)?
            .checkout_control(handle.control_index)
    }

    pub(super) fn finish_bytecode_control(&mut self, handle: BytecodeControlHandle) -> Result<()> {
        self.bytecode_control_continuation_mut(handle)?
            .finish_control(handle.control_index)?;
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
    }

    pub(super) fn park_bytecode_control(
        &mut self,
        handle: BytecodeControlHandle,
        record: BytecodeControlRecord,
    ) -> Result<()> {
        self.activation_frames
            .get_mut(handle.activation_index)
            .map(ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::runtime("structured control continuation disappeared"))?
            .park_control(handle.control_index, record)
    }

    pub(super) fn run_bytecode_control_segment<T>(
        &mut self,
        handle: BytecodeControlHandle,
        record: &mut BytecodeControlRecord,
        slot: BytecodeControlStateSlot,
        run: impl FnOnce(&mut Self, &mut BytecodeState) -> Result<T>,
    ) -> Result<T> {
        match self.run_bytecode_control_segment_result(record, slot, run) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.finish_bytecode_control(handle)?;
                Err(error)
            }
        }
    }

    pub(super) fn run_bytecode_control_segment_result<T>(
        &mut self,
        record: &mut BytecodeControlRecord,
        slot: BytecodeControlStateSlot,
        run: impl FnOnce(&mut Self, &mut BytecodeState) -> Result<T>,
    ) -> Result<T> {
        let _root_scope = if record.has_traceable_transient_roots() {
            Some(self.transient_root_scope(
                crate::runtime::roots::VmRootKind::TransientTemporary,
                record.transient_root_values(),
            )?)
        } else {
            None
        };
        run(self, record.state_mut(slot)?)
    }

    pub(super) fn run_bytecode_control_action<T>(
        &mut self,
        handle: BytecodeControlHandle,
        record: &BytecodeControlRecord,
        run: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        match self.run_bytecode_control_action_result(record, run) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.finish_bytecode_control(handle)?;
                Err(error)
            }
        }
    }

    pub(super) fn run_bytecode_control_action_result<T>(
        &mut self,
        record: &BytecodeControlRecord,
        run: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let _root_scope = if record.has_traceable_transient_roots() {
            Some(self.transient_root_scope(
                crate::runtime::roots::VmRootKind::TransientTemporary,
                record.transient_root_values(),
            )?)
        } else {
            None
        };
        run(self)
    }

    pub(super) fn run_bytecode_iterator_action<T>(
        &mut self,
        handle: BytecodeControlHandle,
        record: &mut BytecodeControlRecord,
        run: impl FnOnce(&mut Self, &mut IteratorSource) -> Result<T>,
    ) -> Result<T> {
        let _root_scope = if record.has_traceable_transient_roots() {
            Some(self.transient_root_scope(
                crate::runtime::roots::VmRootKind::TransientTemporary,
                record.transient_root_values(),
            )?)
        } else {
            None
        };
        let result = run(self, record.iterator_source_mut()?);
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.finish_bytecode_control(handle)?;
                Err(error)
            }
        }
    }

    pub(super) fn finish_bytecode_control_result<T>(
        &mut self,
        handle: BytecodeControlHandle,
        result: Result<T>,
    ) -> Result<T> {
        self.finish_bytecode_control(handle)?;
        result
    }

    fn bytecode_control_continuation_mut(
        &mut self,
        handle: BytecodeControlHandle,
    ) -> Result<&mut super::BytecodeContinuationFrame> {
        let expected = self
            .activation_frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("structured control activation stack is empty"))?;
        if handle.activation_index != expected {
            return Err(Error::runtime("structured control activation mismatch"));
        }
        self.activation_frames
            .get_mut(handle.activation_index)
            .map(ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::runtime("structured control continuation disappeared"))
    }
}

const fn completion_value(completion: &Completion) -> Option<&Value> {
    match completion {
        Completion::Normal(value)
        | Completion::Throw(value)
        | Completion::Return(value)
        | Completion::Break { value, .. } => Some(value),
        Completion::Continue(_) => None,
        Completion::Suspended(_) => None,
    }
}
