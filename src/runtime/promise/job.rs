use crate::{
    error::Result,
    runtime::{
        async_operation::ArrayFromAsyncContinuation,
        async_trace::VmAsyncEdgeKind,
        function::SuspendedAsyncFunction,
        generator::GeneratorId,
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
    AsyncGeneratorAwait {
        generator: GeneratorId,
    },
    ArrayFromAsync {
        continuation: Box<ArrayFromAsyncContinuation>,
    },
}

impl PromiseReaction {
    pub(in crate::runtime) const fn new(
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

    pub(in crate::runtime) const fn awaiting_async_generator(generator: GeneratorId) -> Self {
        Self::AsyncGeneratorAwait { generator }
    }

    pub(in crate::runtime) fn awaiting_array_from_async(
        continuation: ArrayFromAsyncContinuation,
    ) -> Self {
        Self::ArrayFromAsync {
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
            Self::AsyncGeneratorAwait { .. } => {}
            Self::ArrayFromAsync { continuation } => {
                continuation.visit_strong_edges(visitor)?;
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn execution_frame_count(&self) -> Result<usize> {
        match self {
            Self::Await { continuation } => continuation.execution_frame_count(),
            Self::Then { .. } | Self::AsyncGeneratorAwait { .. } | Self::ArrayFromAsync { .. } => {
                Ok(0)
            }
        }
    }

    pub(in crate::runtime) fn cache_entry_count(&self) -> Result<usize> {
        match self {
            Self::Await { continuation } => continuation.cache_entry_count(),
            Self::Then { .. } | Self::AsyncGeneratorAwait { .. } | Self::ArrayFromAsync { .. } => {
                Ok(0)
            }
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
            Self::AsyncGeneratorAwait { .. } => Ok(()),
            Self::ArrayFromAsync { continuation } => continuation.visit_direct_roots(visitor),
        }
    }

    pub(in crate::runtime) fn into_cancellation(self) -> Option<PromiseContinuationCancellation> {
        match self {
            Self::Then { .. } => None,
            Self::Await { continuation } => Some(PromiseContinuationCancellation::AsyncFunction(
                *continuation,
            )),
            Self::AsyncGeneratorAwait { generator } => {
                Some(PromiseContinuationCancellation::AsyncGenerator(generator))
            }
            Self::ArrayFromAsync { .. } => None,
        }
    }

    pub(in crate::runtime) fn into_suspended(self) -> Option<SuspendedAsyncFunction> {
        match self {
            Self::Await { continuation } => Some(*continuation),
            Self::Then { .. } | Self::AsyncGeneratorAwait { .. } | Self::ArrayFromAsync { .. } => {
                None
            }
        }
    }
}

#[derive(Debug)]
pub(in crate::runtime) enum PromiseContinuationCancellation {
    AsyncFunction(SuspendedAsyncFunction),
    AsyncGenerator(GeneratorId),
}

#[derive(Debug)]
pub(in crate::runtime) enum PromiseJob {
    Reaction {
        reaction: PromiseReaction,
        state: PromiseSettledState,
    },
    ResolveThenable {
        promise: PromiseId,
        thenable: Value,
        then: Value,
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
            Self::ResolveThenable {
                promise,
                thenable,
                then,
            } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *promise)?;
                visitor.visit_value(VmRootKind::QueuedJob, thenable)?;
                visitor.visit_value(VmRootKind::QueuedJob, then)
            }
        }
    }

    pub(in crate::runtime) fn execution_frame_count(&self) -> Result<usize> {
        match self {
            Self::Reaction { reaction, .. } => reaction.execution_frame_count(),
            Self::ResolveThenable { .. } => Ok(0),
        }
    }

    pub(in crate::runtime) fn cache_entry_count(&self) -> Result<usize> {
        match self {
            Self::Reaction { reaction, .. } => reaction.cache_entry_count(),
            Self::ResolveThenable { .. } => Ok(0),
        }
    }

    pub(in crate::runtime) fn into_cancellation(self) -> Option<PromiseContinuationCancellation> {
        match self {
            Self::Reaction { reaction, .. } => reaction.into_cancellation(),
            Self::ResolveThenable { .. } => None,
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
