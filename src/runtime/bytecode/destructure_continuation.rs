use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeBinding, BytecodePattern, BytecodePatternKey, BytecodePatternProperty,
        BytecodePatternTarget,
    },
    runtime::abstract_operations::IteratorSource,
    syntax::DeclKind,
    value::Value,
};

#[derive(Debug)]
pub(super) struct DestructureContinuation {
    pub(super) kind: DeclKind,
    pub(super) tasks: Vec<DestructureTask>,
}

impl DestructureContinuation {
    pub(super) fn new(pattern: BytecodePattern, kind: DeclKind, value: Value) -> Self {
        Self {
            kind,
            tasks: vec![DestructureTask::Pattern { pattern, value }],
        }
    }

    pub(super) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.tasks.iter().flat_map(DestructureTask::root_values)
    }
}

#[derive(Debug)]
pub(super) enum DestructureTask {
    Pattern {
        pattern: BytecodePattern,
        value: Value,
    },
    Object {
        properties: Rc<[BytecodePatternProperty]>,
        rest: Option<BytecodeBinding>,
        source: Value,
        next: usize,
        consumed: Vec<String>,
    },
    ObjectProperty {
        key: BytecodePatternKey,
        target: BytecodePatternTarget,
        source: Value,
        phase: ObjectPropertyPhase,
    },
    Array {
        elements: Rc<[Option<BytecodePatternTarget>]>,
        rest: Option<Rc<BytecodePattern>>,
        source: IteratorSource,
        next: usize,
        exhausted: bool,
    },
    ArrayElement {
        target: BytecodePatternTarget,
        value: Value,
    },
}

impl DestructureTask {
    fn root_values(&self) -> Vec<&Value> {
        match self {
            Self::Pattern { value, .. }
            | Self::Object { source: value, .. }
            | Self::ObjectProperty { source: value, .. }
            | Self::ArrayElement { value, .. } => vec![value],
            Self::Array { source, .. } => source.root_values().collect(),
        }
    }
}

#[derive(Debug)]
pub(super) enum ObjectPropertyPhase {
    Read,
    Default { value: Value },
}
