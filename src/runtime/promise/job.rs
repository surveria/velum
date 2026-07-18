use core::time::Duration;

use crate::{
    error::Result,
    runtime::{
        async_disposable_stack::AsyncDisposableStackContinuation,
        async_operation::ArrayFromAsyncContinuation,
        async_trace::VmAsyncEdgeKind,
        dynamic_import::DynamicImportJob,
        function::{SuspendedAsyncFunction, SuspendedExecutionStorageFootprint},
        generator::GeneratorId,
        host_command::HostCommandCompletion,
        object::AtomicWaitRegistration,
        resource_scope::ResourceScopeContinuation,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

use super::state::PromiseId;

#[derive(Debug)]
pub(in crate::runtime) enum PromiseReactionResult {
    Intrinsic(PromiseId),
    Capability { resolve: Value, reject: Value },
}

#[derive(Debug)]
pub(in crate::runtime) enum PromiseReaction {
    Then {
        result: PromiseReactionResult,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    },
    Await {
        continuation: Box<SuspendedAsyncFunction>,
    },
    ModuleAwait {
        module: usize,
    },
    ModuleDependency {
        module: usize,
        dependency: usize,
    },
    ModuleAlias {
        module: usize,
        canonical: usize,
    },
    DynamicImportModule {
        result: PromiseId,
        namespace: Value,
    },
    AsyncGeneratorAwait {
        generator: GeneratorId,
    },
    AsyncFromSync {
        result: PromiseId,
        iterator: Value,
    },
    AsyncIteratorDispose {
        result: PromiseId,
    },
    ArrayFromAsync {
        continuation: Box<ArrayFromAsyncContinuation>,
    },
    AsyncDisposableStack {
        continuation: Box<AsyncDisposableStackContinuation>,
    },
    ResourceScope {
        continuation: Box<ResourceScopeContinuation>,
    },
    HostCommand {
        completion: HostCommandCompletion,
    },
}

impl PromiseReaction {
    pub(in crate::runtime) const fn new(
        result: PromiseId,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    ) -> Self {
        Self::Then {
            result: PromiseReactionResult::Intrinsic(result),
            on_fulfilled,
            on_rejected,
        }
    }

    pub(in crate::runtime) const fn with_capability(
        resolve: Value,
        reject: Value,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    ) -> Self {
        Self::Then {
            result: PromiseReactionResult::Capability { resolve, reject },
            on_fulfilled,
            on_rejected,
        }
    }

    pub(super) fn awaiting(continuation: SuspendedAsyncFunction) -> Self {
        Self::Await {
            continuation: Box::new(continuation),
        }
    }

    pub(in crate::runtime) const fn module_await(module: usize) -> Self {
        Self::ModuleAwait { module }
    }

    pub(in crate::runtime) const fn module_dependency(module: usize, dependency: usize) -> Self {
        Self::ModuleDependency { module, dependency }
    }

    pub(in crate::runtime) const fn module_alias(module: usize, canonical: usize) -> Self {
        Self::ModuleAlias { module, canonical }
    }

    pub(in crate::runtime) const fn dynamic_import_module(
        result: PromiseId,
        namespace: Value,
    ) -> Self {
        Self::DynamicImportModule { result, namespace }
    }

    pub(in crate::runtime) const fn awaiting_async_generator(generator: GeneratorId) -> Self {
        Self::AsyncGeneratorAwait { generator }
    }

    pub(in crate::runtime) const fn async_from_sync(result: PromiseId, iterator: Value) -> Self {
        Self::AsyncFromSync { result, iterator }
    }

    pub(in crate::runtime) const fn async_iterator_dispose(result: PromiseId) -> Self {
        Self::AsyncIteratorDispose { result }
    }

    pub(in crate::runtime) fn awaiting_array_from_async(
        continuation: ArrayFromAsyncContinuation,
    ) -> Self {
        Self::ArrayFromAsync {
            continuation: Box::new(continuation),
        }
    }

    pub(in crate::runtime) fn awaiting_async_disposable_stack(
        continuation: AsyncDisposableStackContinuation,
    ) -> Self {
        Self::AsyncDisposableStack {
            continuation: Box::new(continuation),
        }
    }

    pub(in crate::runtime) fn awaiting_resource_scope(
        continuation: ResourceScopeContinuation,
    ) -> Self {
        Self::ResourceScope {
            continuation: Box::new(continuation),
        }
    }

    pub(in crate::runtime) const fn host_command(completion: HostCommandCompletion) -> Self {
        Self::HostCommand { completion }
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
                match result {
                    PromiseReactionResult::Intrinsic(result) => visitor.visit(
                        VmAsyncEdgeKind::PromiseReaction,
                        StrongEdgeReference::Promise(*result),
                    )?,
                    PromiseReactionResult::Capability { resolve, reject } => {
                        visitor.visit(
                            VmAsyncEdgeKind::PromiseReaction,
                            StrongEdgeReference::Value(resolve),
                        )?;
                        visitor.visit(
                            VmAsyncEdgeKind::PromiseReaction,
                            StrongEdgeReference::Value(reject),
                        )?;
                    }
                }
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
            Self::DynamicImportModule { result, namespace } => {
                visitor.visit(
                    VmAsyncEdgeKind::PromiseReaction,
                    StrongEdgeReference::Promise(*result),
                )?;
                visitor.visit(
                    VmAsyncEdgeKind::PromiseReaction,
                    StrongEdgeReference::Value(namespace),
                )?;
            }
            Self::ModuleAwait { .. }
            | Self::ModuleDependency { .. }
            | Self::ModuleAlias { .. }
            | Self::AsyncGeneratorAwait { .. }
            | Self::HostCommand { .. } => {}
            Self::AsyncFromSync { result, iterator } => {
                visitor.visit(
                    VmAsyncEdgeKind::PromiseReaction,
                    StrongEdgeReference::Promise(*result),
                )?;
                visitor.visit(
                    VmAsyncEdgeKind::PromiseReaction,
                    StrongEdgeReference::Value(iterator),
                )?;
            }
            Self::AsyncIteratorDispose { result } => visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Promise(*result),
            )?,
            Self::ArrayFromAsync { continuation } => {
                continuation.visit_strong_edges(visitor)?;
            }
            Self::AsyncDisposableStack { continuation } => {
                continuation.visit_strong_edges(visitor)?;
            }
            Self::ResourceScope { continuation } => {
                continuation.visit_strong_edges(visitor)?;
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn suspended_execution_storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        match self {
            Self::Await { continuation } => continuation.storage_footprint(),
            Self::Then { .. }
            | Self::ModuleAwait { .. }
            | Self::ModuleDependency { .. }
            | Self::ModuleAlias { .. }
            | Self::DynamicImportModule { .. }
            | Self::AsyncGeneratorAwait { .. }
            | Self::AsyncFromSync { .. }
            | Self::AsyncIteratorDispose { .. }
            | Self::ArrayFromAsync { .. }
            | Self::AsyncDisposableStack { .. }
            | Self::ResourceScope { .. }
            | Self::HostCommand { .. } => Ok(SuspendedExecutionStorageFootprint::default()),
        }
    }

    fn visit_direct_roots<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        match self {
            Self::Then {
                result,
                on_fulfilled,
                on_rejected,
            } => {
                match result {
                    PromiseReactionResult::Intrinsic(result) => {
                        visitor.visit_promise(VmRootKind::QueuedJob, *result)?;
                    }
                    PromiseReactionResult::Capability { resolve, reject } => {
                        visitor.visit_value(VmRootKind::QueuedJob, resolve)?;
                        visitor.visit_value(VmRootKind::QueuedJob, reject)?;
                    }
                }
                if let Some(value) = on_fulfilled {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                if let Some(value) = on_rejected {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
                Ok(())
            }
            Self::Await { continuation } => continuation.visit_direct_roots(visitor),
            Self::DynamicImportModule { result, namespace } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *result)?;
                visitor.visit_value(VmRootKind::QueuedJob, namespace)
            }
            Self::ModuleAwait { .. } | Self::ModuleDependency { .. } | Self::ModuleAlias { .. } => {
                Ok(())
            }
            Self::AsyncGeneratorAwait { .. } | Self::HostCommand { .. } => Ok(()),
            Self::AsyncFromSync { result, iterator } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *result)?;
                visitor.visit_value(VmRootKind::QueuedJob, iterator)
            }
            Self::AsyncIteratorDispose { result } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *result)
            }
            Self::ArrayFromAsync { continuation } => continuation.visit_direct_roots(visitor),
            Self::AsyncDisposableStack { continuation } => continuation.visit_direct_roots(visitor),
            Self::ResourceScope { continuation } => continuation.visit_direct_roots(visitor),
        }
    }

    pub(in crate::runtime) fn into_cancellation(self) -> Option<PromiseContinuationCancellation> {
        match self {
            Self::Await { continuation } => Some(PromiseContinuationCancellation::AsyncFunction(
                *continuation,
            )),
            Self::AsyncGeneratorAwait { generator } => {
                Some(PromiseContinuationCancellation::AsyncGenerator(generator))
            }
            Self::ModuleAwait { module }
            | Self::ModuleDependency { module, .. }
            | Self::ModuleAlias { module, .. } => {
                Some(PromiseContinuationCancellation::ModuleEvaluation(module))
            }
            Self::HostCommand { completion } => {
                Some(PromiseContinuationCancellation::HostCommand(completion))
            }
            Self::Then { .. }
            | Self::DynamicImportModule { .. }
            | Self::AsyncFromSync { .. }
            | Self::AsyncIteratorDispose { .. }
            | Self::ArrayFromAsync { .. }
            | Self::AsyncDisposableStack { .. }
            | Self::ResourceScope { .. } => None,
        }
    }

    pub(in crate::runtime) fn into_suspended(self) -> Option<SuspendedAsyncFunction> {
        match self {
            Self::Await { continuation } => Some(*continuation),
            Self::Then { .. }
            | Self::ModuleAwait { .. }
            | Self::ModuleDependency { .. }
            | Self::ModuleAlias { .. }
            | Self::DynamicImportModule { .. }
            | Self::AsyncGeneratorAwait { .. }
            | Self::AsyncFromSync { .. }
            | Self::AsyncIteratorDispose { .. }
            | Self::ArrayFromAsync { .. }
            | Self::AsyncDisposableStack { .. }
            | Self::ResourceScope { .. }
            | Self::HostCommand { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(in crate::runtime) enum PromiseContinuationCancellation {
    AsyncFunction(SuspendedAsyncFunction),
    AsyncGenerator(GeneratorId),
    ModuleEvaluation(usize),
    HostCommand(HostCommandCompletion),
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
    DynamicImport(DynamicImportJob),
    AtomicsWait {
        promise: PromiseId,
        registration: AtomicWaitRegistration,
        timeout: Option<Duration>,
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
            Self::DynamicImport(job) => visitor.visit_promise(VmRootKind::QueuedJob, job.promise()),
            Self::AtomicsWait { promise, .. } => {
                visitor.visit_promise(VmRootKind::QueuedJob, *promise)
            }
        }
    }

    pub(in crate::runtime) fn suspended_execution_storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        match self {
            Self::Reaction { reaction, .. } => reaction.suspended_execution_storage_footprint(),
            Self::ResolveThenable { .. } | Self::DynamicImport(_) | Self::AtomicsWait { .. } => {
                Ok(SuspendedExecutionStorageFootprint::default())
            }
        }
    }

    pub(in crate::runtime) fn into_cancellation(self) -> Option<PromiseContinuationCancellation> {
        match self {
            Self::Reaction { reaction, .. } => reaction.into_cancellation(),
            Self::ResolveThenable { .. } | Self::DynamicImport(_) | Self::AtomicsWait { .. } => {
                None
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

    pub(in crate::runtime) fn into_completion(self) -> crate::runtime::control::Completion {
        match self.status {
            PromiseStatus::Fulfilled => crate::runtime::control::Completion::Normal(self.value),
            PromiseStatus::Rejected => crate::runtime::control::Completion::Throw(self.value),
        }
    }

    pub(in crate::runtime) const fn rejection_value(&self) -> Option<&Value> {
        match self.status {
            PromiseStatus::Fulfilled => None,
            PromiseStatus::Rejected => Some(&self.value),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PromiseStatus {
    Fulfilled,
    Rejected,
}
