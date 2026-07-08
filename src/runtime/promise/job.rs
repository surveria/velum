use crate::value::Value;

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
}

#[derive(Debug, Clone)]
pub(in crate::runtime) enum PromiseJob {
    Reaction {
        reaction: PromiseReaction,
        state: PromiseSettledState,
    },
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
