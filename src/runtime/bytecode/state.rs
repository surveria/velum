use crate::{
    bytecode::{BytecodeAddress, BytecodeCompletion},
    error::{Error, Result},
    runtime::control::Completion,
    syntax::StaticName,
    value::Value,
};

#[derive(Debug, Clone)]
pub(in crate::runtime) struct BytecodeState {
    pub(super) pc: BytecodeAddress,
    pub(super) stack: BytecodeStack,
    pub(super) last: Value,
}

impl BytecodeState {
    pub(in crate::runtime) const fn new() -> Self {
        Self {
            pc: BytecodeAddress::new(0),
            stack: BytecodeStack::new(),
            last: Value::Undefined,
        }
    }

    pub(super) fn reset(&mut self) {
        self.pc = BytecodeAddress::new(0);
        self.stack.clear();
        self.last = Value::Undefined;
    }

    pub(super) fn next_pc(&self) -> Result<BytecodeAddress> {
        let next = self
            .pc
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::runtime("bytecode instruction pointer overflowed"))?;
        Ok(BytecodeAddress::new(next))
    }

    pub(super) fn complete(&mut self, completion: BytecodeCompletion) -> Result<Completion> {
        match completion {
            BytecodeCompletion::Break(label) => Ok(Completion::Break {
                label,
                value: self.last.clone(),
            }),
            BytecodeCompletion::Continue(label) => Ok(Completion::Continue(label)),
            BytecodeCompletion::Return => Ok(Completion::Return(self.stack.pop_single()?)),
            BytecodeCompletion::Throw => Ok(Completion::Throw(self.stack.pop_single()?)),
        }
    }
}

#[derive(Debug, Clone)]
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

pub(super) fn init_completion_to_result(completion: Completion) -> Result<()> {
    match completion {
        Completion::Normal(_) => Ok(()),
        completion => completion.into_result().map(|_| ()),
    }
}

pub(super) fn bytecode_loop_completion(
    last: &mut Value,
    completion: Completion,
    labels: Option<&[StaticName]>,
) -> Option<Completion> {
    match completion {
        Completion::Normal(value) => {
            *last = value;
            None
        }
        Completion::Continue(None) => None,
        Completion::Break { label: None, value } => Some(Completion::Normal(value)),
        Completion::Continue(Some(target)) if loop_label_matches(labels, &target) => None,
        Completion::Break {
            label: Some(target),
            value,
        } if loop_label_matches(labels, &target) => Some(Completion::Normal(value)),
        completion @ (Completion::Break { .. }
        | Completion::Continue(Some(_))
        | Completion::Throw(_)
        | Completion::Return(_)) => Some(completion),
    }
}

pub(super) fn loop_label_matches(labels: Option<&[StaticName]>, target: &StaticName) -> bool {
    labels.is_some_and(|labels| labels.iter().any(|label| label == target))
}
