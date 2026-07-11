use crate::{
    error::Result,
    runtime::{
        async_trace::VmAsyncEdgeKind,
        function::SuspendedAsyncFunction,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

use super::state::PromiseId;

#[derive(Debug)]
pub(in crate::runtime) enum PromiseReaction {
    Then {
        result: PromiseId,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    },
    Await {
        continuation: Box<SuspendedAsyncFunction>,
    },
}

impl PromiseReaction {
    pub(super) const fn new(
        result: PromiseId,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    ) -> Self {
        Self::Then {
            result,
            on_fulfilled,
            on_rejected,
        }
    }

    pub(super) fn awaiting(continuation: SuspendedAsyncFunction) -> Self {
        Self::Await {
            continuation: Box::new(continuation),
        }
    }

    pub(super) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        match self {
            Self::Then {
                result,
                on_fulfilled,
                on_rejected,
            } => {
                visitor.visit(
                    VmAsyncEdgeKind::PromiseReaction,
                    StrongEdgeReference::Promise(*result),
                )?;
                if let Some(handler) = on_fulfilled {
                    visitor.visit(
                        VmAsyncEdgeKind::PromiseReaction,
                        StrongEdgeReference::Value(handler),
                    )?;
                }
                if let Some(handler) = on_rejected {
                    visitor.visit(
                        VmAsyncEdgeKind::PromiseReaction,
                        StrongEdgeReference::Value(handler),
                    )?;
                }
            }
            Self::Await { continuation } => continuation.visit_strong_edges(visitor)?,
        }
        Ok(())
    }

    pub(in crate::runtime) fn execution_frame_count(&self) -> Result<usize> {
        match self {
            Self::Then { .. } => Ok(0),
            Self::Await { continuation } => continuation.execution_frame_count(),
        }
    }

    pub(in crate::runtime) fn cache_entry_count(&self) -> Result<usize> {
        match self {
            Self::Then { .. } => Ok(0),
            Self::Await { continuation } => continuation.cache_entry_count(),
        }
    }

    fn visit_direct_roots<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        match self {
            Self::Then {
                result,
                on_fulfilled,
                on_rejected,
            } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *result)?;
                if let Some(value) = on_fulfilled {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                if let Some(value) = on_rejected {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                Ok(())
            }
            Self::Await { continuation } => continuation.visit_direct_roots(visitor),
        }
    }

    pub(in crate::runtime) fn into_suspended(self) -> Option<SuspendedAsyncFunction> {
        match self {
            Self::Then { .. } => None,
            Self::Await { continuation } => Some(*continuation),
        }
    }
}

#[derive(Debug)]
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
                reaction.visit_direct_roots(visitor)?;
                visitor.visit_value(VmRootKind::QueuedJob, &state.value)
            }
        }
    }

    pub(in crate::runtime) fn execution_frame_count(&self) -> Result<usize> {
        match self {
            Self::Reaction { reaction, .. } => reaction.execution_frame_count(),
        }
    }

    pub(in crate::runtime) fn cache_entry_count(&self) -> Result<usize> {
        match self {
            Self::Reaction { reaction, .. } => reaction.cache_entry_count(),
        }
    }

    pub(in crate::runtime) fn into_suspended(self) -> Option<SuspendedAsyncFunction> {
        match self {
            Self::Reaction { reaction, .. } => reaction.into_suspended(),
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
