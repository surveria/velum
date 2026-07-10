use crate::{
    error::Result,
    runtime::{
        async_trace::VmAsyncEdgeKind,
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

use super::job::PromiseReaction;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) struct PromiseId(usize);

impl PromiseId {
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum PromiseResolverKind {
    Resolve,
    Reject,
}

#[derive(Debug, Clone)]
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
}

#[derive(Debug, Clone)]
pub(super) enum PromiseState {
    Pending { reactions: Vec<PromiseReaction> },
    Fulfilled(Value),
    Rejected(Value),
}
