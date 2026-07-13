pub(super) const PROMISE_CATCH_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_CATCH_NAME: &str = "catch";
pub(super) const PROMISE_CAPABILITY_EXECUTOR_FUNCTION_LENGTH: f64 = 2.0;
pub(super) const PROMISE_CAPABILITY_EXECUTOR_NAME: &str = "";
pub(super) const PROMISE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_NAME: &str = "Promise";
pub(super) const PROMISE_FINALLY_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_FINALLY_NAME: &str = "finally";
pub(super) const PROMISE_REJECT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_REJECT_NAME: &str = "reject";
pub(super) const PROMISE_RESOLVE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_RESOLVE_NAME: &str = "resolve";
pub(super) const PROMISE_TRY_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_TRY_NAME: &str = "try";
pub(super) const PROMISE_WITH_RESOLVERS_FUNCTION_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const PROMISE_WITH_RESOLVERS_NAME: &str = "withResolvers";
pub(super) const PROMISE_RESOLVER_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const PROMISE_THEN_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const PROMISE_THEN_NAME: &str = "then";
pub(super) const REJECT_NAME: &str = "";
pub(super) const RESOLVE_NAME: &str = "";

impl NativeFunctionKind {
    pub(super) const fn promise_length(self) -> Option<f64> {
        match self {
            Self::Promise | Self::PromiseCombinator(_) => Some(PROMISE_FUNCTION_LENGTH),
            Self::PromiseCapabilityExecutor { .. } => {
                Some(PROMISE_CAPABILITY_EXECUTOR_FUNCTION_LENGTH)
            }
            Self::PromiseResolve => Some(PROMISE_RESOLVE_FUNCTION_LENGTH),
            Self::PromiseReject => Some(PROMISE_REJECT_FUNCTION_LENGTH),
            Self::PromiseTry => Some(PROMISE_TRY_FUNCTION_LENGTH),
            Self::PromiseWithResolvers => Some(PROMISE_WITH_RESOLVERS_FUNCTION_LENGTH),
            Self::PromiseThen => Some(PROMISE_THEN_FUNCTION_LENGTH),
            Self::PromiseCatch => Some(PROMISE_CATCH_FUNCTION_LENGTH),
            Self::PromiseFinally => Some(PROMISE_FINALLY_FUNCTION_LENGTH),
            Self::PromiseFinallyFunction { kind, .. } => Some(kind.length()),
            Self::PromiseResolver { .. } => Some(PROMISE_RESOLVER_FUNCTION_LENGTH),
            _ => None,
        }
    }

    pub(super) const fn promise_name(self) -> Option<&'static str> {
        match self {
            Self::Promise => Some(PROMISE_NAME),
            Self::PromiseCombinator(kind) => Some(kind.name()),
            Self::PromiseCapabilityExecutor { .. } => Some(PROMISE_CAPABILITY_EXECUTOR_NAME),
            Self::PromiseResolve => Some(PROMISE_RESOLVE_NAME),
            Self::PromiseReject => Some(PROMISE_REJECT_NAME),
            Self::PromiseTry => Some(PROMISE_TRY_NAME),
            Self::PromiseWithResolvers => Some(PROMISE_WITH_RESOLVERS_NAME),
            Self::PromiseThen => Some(PROMISE_THEN_NAME),
            Self::PromiseCatch => Some(PROMISE_CATCH_NAME),
            Self::PromiseFinally => Some(PROMISE_FINALLY_NAME),
            Self::PromiseFinallyFunction { .. } => Some(""),
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Resolve,
                ..
            } => Some(RESOLVE_NAME),
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Reject,
                ..
            } => Some(REJECT_NAME),
            _ => None,
        }
    }
}
use super::NativeFunctionKind;
