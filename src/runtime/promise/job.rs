use crate::{
    error::Result,
    runtime::{
        async_trace::VmAsyncEdgeKind,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

use super::state::PromiseId;

#[derive(Debug, Clone)]
pub(in crate::runtime) struct PromiseReaction {
    pub(super) result: PromiseId,
    pub(super) on_fulfilled: Option<Value>,
    pub(super) on_rejected: Option<Value>,
}

impl PromiseReaction {
    pub(super) const fn new(
        result: PromiseId,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    ) -> Self {
        Self {
            result,
            on_fulfilled,
            on_rejected,
        }
    }

    pub(super) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Promise(self.result),
        )?;
        if let Some(handler) = &self.on_fulfilled {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(handler),
            )?;
        }
        if let Some(handler) = &self.on_rejected {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(handler),
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) enum PromiseJob {
    Reaction {
        reaction: PromiseReaction,
        state: PromiseSettledState,
    },
}

impl PromiseJob {
    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        match self {
            Self::Reaction { reaction, state } => {
                visitor.visit_promise(VmRootKind::QueuedJob, reaction.result)?;
                if let Some(value) = &reaction.on_fulfilled {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                if let Some(value) = &reaction.on_rejected {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                visitor.visit_value(VmRootKind::QueuedJob, &state.value)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct PromiseSettledState {
    pub(super) status: PromiseStatus,
    pub(super) value: Value,
}

impl PromiseSettledState {
    pub(super) const fn fulfilled(value: Value) -> Self {
        Self {
            status: PromiseStatus::Fulfilled,
            value,
        }
    }

    pub(super) const fn rejected(value: Value) -> Self {
        Self {
            status: PromiseStatus::Rejected,
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PromiseStatus {
    Fulfilled,
    Rejected,
}
