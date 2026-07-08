use crate::value::Value;

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
}

#[derive(Debug, Clone)]
pub(super) enum PromiseState {
    Pending { reactions: Vec<PromiseReaction> },
    Fulfilled(Value),
    Rejected(Value),
}
