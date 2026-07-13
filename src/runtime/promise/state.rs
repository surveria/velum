use crate::{
    error::Result,
    runtime::{
        async_trace::VmAsyncEdgeKind,
        function::SuspendedExecutionStorageFootprint,
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

use super::job::PromiseReaction;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PromiseId(usize);

impl PromiseId {
    pub(in crate::runtime) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(in crate::runtime) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseResolverKind {
    Resolve,
    Reject,
}

#[derive(Debug)]
pub(in crate::runtime) struct Promise {
    pub(super) state: PromiseState,
}

impl Promise {
    pub(super) const fn pending() -> Self {
        Self {
            state: PromiseState::Pending {
                reactions: Vec::new(),
            },
        }
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        match &self.state {
            PromiseState::Pending { reactions } => {
                for reaction in reactions {
                    reaction.visit_strong_edges(visitor)?;
                }
            }
            PromiseState::Fulfilled(value) | PromiseState::Rejected(value) => visitor.visit(
                VmAsyncEdgeKind::PromiseState,
                StrongEdgeReference::Value(value),
            )?,
        }
        Ok(())
    }

    pub(in crate::runtime) fn suspended_execution_storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        let PromiseState::Pending { reactions } = &self.state else {
            return Ok(SuspendedExecutionStorageFootprint::default());
        };
        reactions.iter().try_fold(
            SuspendedExecutionStorageFootprint::default(),
            |footprint, reaction| {
                footprint.checked_add(reaction.suspended_execution_storage_footprint()?)
            },
        )
    }
}

#[derive(Debug)]
pub(super) enum PromiseState {
    Pending { reactions: Vec<PromiseReaction> },
    Fulfilled(Value),
    Rejected(Value),
}
