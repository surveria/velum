use std::rc::Rc;

use crate::{
    bytecode::{BytecodeAddress, BytecodeCompletion},
    error::{Error, Result},
    runtime::{
        abstract_operations::YieldDelegateContinuation, control::Completion,
        private::PrivateEnvironment,
    },
    syntax::StaticName,
    value::Value,
};

use super::destructure_continuation::DestructureContinuation;

#[derive(Debug)]
pub(in crate::runtime) struct BytecodeState {
    pub(super) pc: BytecodeAddress,
    pub(super) stack: BytecodeStack,
    pub(super) last: Value,
    private_environment: Option<Rc<PrivateEnvironment>>,
    suspend: Option<Box<BytecodeSuspendState>>,
}

#[derive(Debug)]
struct BytecodeSuspendState {
    phase: BytecodeSuspendPhase,
    resume_completion: Option<Completion>,
    destructure: Option<DestructureContinuation>,
    yield_delegate: Option<YieldDelegateContinuation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BytecodeSuspendPhase {
    Running,
    Awaiting,
    GeneratorStarting,
    Yielding,
    ChildSuspended,
    ResumeReady,
}

impl BytecodeState {
    pub(in crate::runtime) const fn new() -> Self {
        Self {
            pc: BytecodeAddress::new(0),
            stack: BytecodeStack::new(),
            last: Value::Undefined,
            private_environment: None,
            suspend: None,
        }
    }

    pub(in crate::runtime) const fn with_private_environment(
        private_environment: Option<Rc<PrivateEnvironment>>,
    ) -> Self {
        Self {
            pc: BytecodeAddress::new(0),
            stack: BytecodeStack::new(),
            last: Value::Undefined,
            private_environment,
            suspend: None,
        }
    }

    pub(super) fn reset(&mut self) {
        self.pc = BytecodeAddress::new(0);
        self.stack.clear();
        self.last = Value::Undefined;
        self.suspend = None;
    }

    pub(in crate::runtime) fn private_environment(&self) -> Option<Rc<PrivateEnvironment>> {
        self.private_environment.clone()
    }

    pub(in crate::runtime) const fn replace_private_environment(
        &mut self,
        environment: Option<Rc<PrivateEnvironment>>,
    ) -> Option<Rc<PrivateEnvironment>> {
        std::mem::replace(&mut self.private_environment, environment)
    }

    pub(super) fn prepare_run(&mut self) -> Result<()> {
        if self.suspend.is_none() {
            self.pc = BytecodeAddress::new(0);
            self.stack.clear();
            self.last = Value::Undefined;
            return Ok(());
        }
        self.prepare_suspended_run()
    }

    #[cold]
    fn prepare_suspended_run(&mut self) -> Result<()> {
        if let Some(suspend) = self.suspend.as_mut() {
            match suspend.phase {
                BytecodeSuspendPhase::Awaiting
                | BytecodeSuspendPhase::GeneratorStarting
                | BytecodeSuspendPhase::Yielding => {
                    return Err(Error::runtime(
                        "suspended bytecode state has no resume value",
                    ));
                }
                BytecodeSuspendPhase::ChildSuspended | BytecodeSuspendPhase::ResumeReady => {
                    suspend.phase = BytecodeSuspendPhase::Running;
                    return Ok(());
                }
                BytecodeSuspendPhase::Running if suspend.destructure.is_some() => return Ok(()),
                BytecodeSuspendPhase::Running => {}
            }
        }
        self.reset();
        Ok(())
    }

    pub(super) const fn has_suspend_state(&self) -> bool {
        self.suspend.is_some()
    }

    pub(super) fn mark_await_suspended(&mut self) {
        self.suspend_state_mut().phase = BytecodeSuspendPhase::Awaiting;
    }

    pub(super) fn mark_yield_suspended(&mut self) {
        self.suspend_state_mut().phase = BytecodeSuspendPhase::Yielding;
    }

    pub(super) fn mark_generator_start_suspended(&mut self) {
        self.suspend_state_mut().phase = BytecodeSuspendPhase::GeneratorStarting;
    }

    pub(super) fn mark_child_suspended(&mut self) {
        self.suspend_state_mut().phase = BytecodeSuspendPhase::ChildSuspended;
    }

    pub(super) fn is_suspended(&self) -> bool {
        self.suspend.as_ref().is_some_and(|suspend| {
            matches!(
                suspend.phase,
                BytecodeSuspendPhase::Awaiting
                    | BytecodeSuspendPhase::GeneratorStarting
                    | BytecodeSuspendPhase::Yielding
                    | BytecodeSuspendPhase::ChildSuspended
            )
        })
    }

    pub(super) fn is_awaiting(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.phase == BytecodeSuspendPhase::Awaiting)
    }

    pub(super) fn is_yielding(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.phase == BytecodeSuspendPhase::Yielding)
    }

    pub(super) fn is_generator_starting(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.phase == BytecodeSuspendPhase::GeneratorStarting)
    }

    pub(super) fn is_resuming(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.phase != BytecodeSuspendPhase::Running)
    }

    pub(super) fn resume_suspension(&mut self, completion: Completion) -> Result<()> {
        if !self.is_awaiting() && !self.is_generator_starting() && !self.is_yielding() {
            return Err(Error::runtime(
                "bytecode state is not awaiting a resume completion",
            ));
        }
        let delegates = self.has_yield_delegate();
        let permits_abrupt = self.is_yielding() || delegates || self.is_generator_starting();
        let discards_normal = self.is_generator_starting();
        let (value, resume_completion) = match completion {
            completion if delegates => (None, Some(completion)),
            Completion::Normal(_) if discards_normal => (None, None),
            Completion::Normal(value) => (Some(value), None),
            completion @ Completion::Throw(_) => (None, Some(completion)),
            completion @ (Completion::Return(_) | Completion::ReturnDirect(_))
                if permits_abrupt =>
            {
                (None, Some(completion))
            }
            completion => return completion.into_result().map(|_| ()),
        };
        let suspend = self.suspend_state_mut();
        suspend.phase = BytecodeSuspendPhase::ResumeReady;
        suspend.resume_completion = resume_completion;
        if let Some(value) = value {
            self.stack.push(value);
        }
        Ok(())
    }

    pub(super) fn take_resume_completion(&mut self) -> Option<Completion> {
        if self.has_yield_delegate() {
            return None;
        }
        let completion = self
            .suspend
            .as_mut()
            .and_then(|suspend| suspend.resume_completion.take());
        self.release_empty_suspend_state();
        completion
    }

    pub(super) fn begin_run(&mut self) {
        if let Some(suspend) = self.suspend.as_mut()
            && matches!(
                suspend.phase,
                BytecodeSuspendPhase::ChildSuspended | BytecodeSuspendPhase::ResumeReady
            )
        {
            suspend.phase = BytecodeSuspendPhase::Running;
        }
    }

    pub(super) fn has_destructure_continuation(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.destructure.is_some())
    }

    pub(super) fn take_destructure_continuation(&mut self) -> Option<DestructureContinuation> {
        let continuation = self
            .suspend
            .as_mut()
            .and_then(|suspend| suspend.destructure.take());
        self.release_empty_suspend_state();
        continuation
    }

    pub(super) fn store_destructure_continuation(
        &mut self,
        continuation: DestructureContinuation,
    ) -> Result<()> {
        let suspend = self.suspend_state_mut();
        if suspend.destructure.is_some() {
            return Err(Error::runtime(
                "bytecode destructuring continuation is already stored",
            ));
        }
        suspend.destructure = Some(continuation);
        Ok(())
    }

    pub(super) fn has_yield_delegate(&self) -> bool {
        self.suspend
            .as_ref()
            .is_some_and(|suspend| suspend.yield_delegate.is_some())
    }

    pub(super) fn take_yield_delegate(
        &mut self,
    ) -> Option<(YieldDelegateContinuation, Option<Completion>)> {
        let suspend = self.suspend.as_mut()?;
        let continuation = suspend.yield_delegate.take()?;
        let resume = suspend.resume_completion.take();
        Some((continuation, resume))
    }

    pub(super) fn store_yield_delegate(
        &mut self,
        continuation: YieldDelegateContinuation,
    ) -> Result<()> {
        let suspend = self.suspend_state_mut();
        if suspend.yield_delegate.is_some() {
            return Err(Error::runtime(
                "bytecode yield delegation continuation is already stored",
            ));
        }
        suspend.yield_delegate = Some(continuation);
        Ok(())
    }

    fn suspend_state_mut(&mut self) -> &mut BytecodeSuspendState {
        self.suspend.get_or_insert_with(|| {
            Box::new(BytecodeSuspendState {
                phase: BytecodeSuspendPhase::Running,
                resume_completion: None,
                destructure: None,
                yield_delegate: None,
            })
        })
    }

    fn release_empty_suspend_state(&mut self) {
        if self.suspend.as_ref().is_some_and(|suspend| {
            suspend.phase == BytecodeSuspendPhase::Running
                && suspend.resume_completion.is_none()
                && suspend.destructure.is_none()
                && suspend.yield_delegate.is_none()
        }) {
            self.suspend = None;
        }
    }

    pub(super) fn next_pc(&self) -> Result<BytecodeAddress> {
        let next = self
            .pc
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::runtime("bytecode instruction pointer overflowed"))?;
        Ok(BytecodeAddress::new(next))
    }

    pub(in crate::runtime) fn root_values(&self) -> BytecodeStateRootValues<'_> {
        let mut cold = Vec::new();
        if let Some(suspend) = self.suspend.as_ref() {
            if let Some(value) = suspend
                .resume_completion
                .as_ref()
                .and_then(completion_value)
            {
                cold.push(value);
            }
            if let Some(destructure) = suspend.destructure.as_ref() {
                cold.extend(destructure.root_values());
            }
            if let Some(delegate) = suspend.yield_delegate.as_ref() {
                cold.extend(delegate.root_values());
            }
        }
        self.root_values_with_cold(cold)
    }

    pub(super) fn synchronous_root_values(&self) -> impl Iterator<Item = &Value> {
        self.stack
            .values()
            .iter()
            .chain(std::iter::once(&self.last))
    }

    fn root_values_with_cold<'state>(
        &'state self,
        cold: Vec<&'state Value>,
    ) -> BytecodeStateRootValues<'state> {
        BytecodeStateRootValues {
            hot: self
                .stack
                .values()
                .iter()
                .chain(std::iter::once(&self.last)),
            cold: if cold.is_empty() {
                None
            } else {
                Some(cold.into_iter())
            },
        }
    }

    pub(super) fn complete(&mut self, completion: BytecodeCompletion) -> Result<Completion> {
        match completion {
            BytecodeCompletion::Break(label) => Ok(Completion::Break {
                label,
                value: self.last.clone(),
            }),
            BytecodeCompletion::Continue(label) => Ok(Completion::Continue {
                label,
                value: self.last.clone(),
            }),
            BytecodeCompletion::Return => Ok(Completion::Return(self.stack.pop_single()?)),
            BytecodeCompletion::ReturnDirect => {
                Ok(Completion::ReturnDirect(self.stack.pop_single()?))
            }
            BytecodeCompletion::Throw => Ok(Completion::Throw(self.stack.pop_single()?)),
        }
    }
}

pub(in crate::runtime) struct BytecodeStateRootValues<'state> {
    hot: std::iter::Chain<std::slice::Iter<'state, Value>, std::iter::Once<&'state Value>>,
    cold: Option<std::vec::IntoIter<&'state Value>>,
}

impl<'state> Iterator for BytecodeStateRootValues<'state> {
    type Item = &'state Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.hot
            .next()
            .or_else(|| self.cold.as_mut().and_then(Iterator::next))
    }
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

#[derive(Clone, Debug)]
pub(super) struct BytecodeStack {
    values: Vec<Value>,
}

impl BytecodeStack {
    const fn new() -> Self {
        Self { values: Vec::new() }
    }

    pub(super) fn push(&mut self, value: Value) {
        self.values.push(value);
    }

    const fn values(&self) -> &[Value] {
        self.values.as_slice()
    }

    fn clear(&mut self) {
        self.values.clear();
    }

    pub(super) fn pop(&mut self) -> Result<Value> {
        self.values
            .pop()
            .ok_or_else(|| Error::runtime("bytecode stack underflowed"))
    }

    pub(super) fn peek(&self) -> Result<&Value> {
        self.values
            .last()
            .ok_or_else(|| Error::runtime("bytecode stack underflowed"))
    }

    pub(super) fn tail(&self, count: usize) -> Result<&[Value]> {
        let start = self.tail_start(count)?;
        self.values
            .get(start..)
            .ok_or_else(|| Error::runtime("bytecode stack tail is not available"))
    }

    pub(super) fn value_before_tail(&self, count: usize, offset: usize) -> Result<&Value> {
        let tail_start = self.tail_start(count)?;
        let before_tail = offset
            .checked_add(1)
            .ok_or_else(|| Error::runtime("bytecode stack offset overflowed"))?;
        let index = tail_start
            .checked_sub(before_tail)
            .ok_or_else(|| Error::runtime("bytecode stack underflowed"))?;
        self.values
            .get(index)
            .ok_or_else(|| Error::runtime("bytecode stack value is not available"))
    }

    pub(super) fn drop_tail(&mut self, count: usize) -> Result<()> {
        let start = self.tail_start(count)?;
        self.values.truncate(start);
        Ok(())
    }

    pub(super) fn pop_many(&mut self, count: usize) -> Result<Vec<Value>> {
        let start = self.tail_start(count)?;
        Ok(self.values.split_off(start))
    }

    pub(super) fn drain_tail(&mut self, count: usize) -> Result<std::vec::Drain<'_, Value>> {
        let start = self.tail_start(count)?;
        Ok(self.values.drain(start..))
    }

    fn tail_start(&self, count: usize) -> Result<usize> {
        self.values
            .len()
            .checked_sub(count)
            .ok_or_else(|| Error::runtime("bytecode stack underflowed"))
    }

    fn pop_single(&mut self) -> Result<Value> {
        let value = self.pop()?;
        if !self.values.is_empty() {
            return Err(Error::runtime(
                "bytecode completion left extra stack values",
            ));
        }
        Ok(value)
    }
}

pub(super) fn bytecode_loop_completion(
    last: &mut Value,
    completion: Completion,
    labels: Option<&[StaticName]>,
) -> Option<Completion> {
    match completion {
        Completion::Normal(value) | Completion::Continue { label: None, value } => {
            *last = value;
            None
        }
        Completion::Break { label: None, value } => Some(Completion::Normal(value)),
        Completion::Continue {
            label: Some(target),
            value,
        } if loop_label_matches(labels, &target) => {
            *last = value;
            None
        }
        Completion::Break {
            label: Some(target),
            value,
        } if loop_label_matches(labels, &target) => Some(Completion::Normal(value)),
        completion @ (Completion::Break { .. }
        | Completion::Continue { label: Some(_), .. }
        | Completion::Throw(_)
        | Completion::Return(_)
        | Completion::ReturnDirect(_)
        | Completion::Suspended(_)
        | Completion::GeneratorStart
        | Completion::Yielded(_)
        | Completion::YieldedIteratorResult(_)) => Some(completion),
    }
}

pub(super) fn loop_label_matches(labels: Option<&[StaticName]>, target: &StaticName) -> bool {
    labels.is_some_and(|labels| labels.iter().any(|label| label == target))
}
