#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::{
    bytecode::{
        BytecodeDestructureMode, BytecodePattern, BytecodePatternKey, BytecodePatternProperty,
        BytecodePatternTarget,
    },
    runtime::abstract_operations::IteratorSource,
    runtime::object::PropertyKey,
    value::Value,
};

use super::ops::BytecodeAssignmentReference;

#[derive(Debug)]
pub(super) struct DestructureContinuation {
    pub(super) mode: BytecodeDestructureMode,
    pub(super) tasks: Vec<DestructureTask>,
}

impl DestructureContinuation {
    pub(super) fn new(
        pattern: BytecodePattern,
        mode: BytecodeDestructureMode,
        value: Value,
    ) -> Self {
        Self {
            mode,
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
        rest: Option<Rc<BytecodePattern>>,
        source: Value,
        next: usize,
        consumed: Vec<PropertyKey>,
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
        reference: Option<BytecodeAssignmentReference>,
    },
}

impl DestructureTask {
    fn root_values(&self) -> Vec<&Value> {
        match self {
            Self::Pattern { value, .. } | Self::Object { source: value, .. } => vec![value],
            Self::ObjectProperty { source, phase, .. } => {
                let mut values = vec![source];
                if let ObjectPropertyPhase::Default {
                    reference: Some(reference),
                    ..
                } = phase
                {
                    values.extend(reference.root_values());
                }
                values
            }
            Self::ArrayElement {
                value, reference, ..
            } => {
                let mut values = vec![value];
                if let Some(reference) = reference {
                    values.extend(reference.root_values());
                }
                values
            }
            Self::Array { source, .. } => source.root_values().collect(),
        }
    }
}

#[derive(Debug)]
pub(super) enum ObjectPropertyPhase {
    Read,
    Default {
        value: Value,
        reference: Option<BytecodeAssignmentReference>,
    },
}
