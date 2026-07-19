#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        async_trace::VmAsyncEdgeKind,
        binding::scope::{BindingResourceStack, BindingScope},
        call::RuntimeCallArgs,
        control::Completion,
        native::{AsyncDisposableStackFunctionKind, DisposableStackFunctionKind},
        promise::{PromiseId, PromiseReaction},
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

pub(in crate::runtime) enum ScopeDisposal {
    Complete(Completion),
    Await(PromiseId),
}

#[derive(Debug)]
pub(in crate::runtime) struct ResourceScopeContinuation {
    result_promise: PromiseId,
    resources: Vec<BindingResourceStack>,
    thrown: Option<Value>,
}

impl ResourceScopeContinuation {
    fn new(
        result_promise: PromiseId,
        resources: Vec<BindingResourceStack>,
        completion: &Completion,
    ) -> Self {
        Self {
            result_promise,
            resources,
            thrown: match completion {
                Completion::Throw(value) => Some(value.clone()),
                _ => None,
            },
        }
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Promise(self.result_promise),
        )?;
        for resource in &self.resources {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(resource.value()),
            )?;
        }
        if let Some(thrown) = &self.thrown {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(thrown),
            )?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit_promise(VmRootKind::QueuedJob, self.result_promise)?;
        for resource in &self.resources {
            visitor.visit_value(VmRootKind::QueuedJob, resource.value())?;
        }
        if let Some(thrown) = &self.thrown {
            visitor.visit_value(VmRootKind::QueuedJob, thrown)?;
        }
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn register_using_resource(&mut self, value: &Value) -> Result<()> {
        let stack = self.construct_disposable_stack()?;
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [&stack, value])?;
        self.eval_disposable_stack_function(
            DisposableStackFunctionKind::Use,
            RuntimeCallArgs::values(core::slice::from_ref(value)),
            &stack,
        )?;
        self.active_binding_scope_mut()?
            .push_resource_stack(BindingResourceStack::Sync(stack));
        Ok(())
    }

    pub(in crate::runtime) fn register_await_using_resource(
        &mut self,
        value: &Value,
    ) -> Result<()> {
        let stack = self.construct_async_disposable_stack()?;
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [&stack, value])?;
        self.eval_async_disposable_stack_function(
            AsyncDisposableStackFunctionKind::Use,
            RuntimeCallArgs::values(core::slice::from_ref(value)),
            &stack,
        )?;
        self.active_binding_scope_mut()?
            .push_resource_stack(BindingResourceStack::Async(stack));
        Ok(())
    }

    fn active_binding_scope_mut(&mut self) -> Result<&mut BindingScope> {
        self.locals
            .last_mut()
            .ok_or_else(|| Error::runtime("resource declaration has no lexical scope"))
    }

    pub(in crate::runtime) fn dispose_active_binding_scope(
        &mut self,
        completion: Completion,
    ) -> Result<Completion> {
        let resources = self
            .locals
            .last_mut()
            .map(BindingScope::take_resource_stacks)
            .unwrap_or_default();
        self.dispose_resource_stacks_sync(resources, completion)
    }

    pub(in crate::runtime) fn begin_dispose_active_binding_scope(
        &mut self,
        completion: Completion,
    ) -> Result<ScopeDisposal> {
        let resources = self
            .locals
            .last_mut()
            .map(BindingScope::take_resource_stacks)
            .unwrap_or_default();
        self.begin_dispose_resource_stacks(resources, completion)
    }

    pub(in crate::runtime) fn begin_dispose_binding_scope(
        &mut self,
        mut scope: BindingScope,
        completion: Completion,
    ) -> Result<ScopeDisposal> {
        self.begin_dispose_resource_stacks(scope.take_resource_stacks(), completion)
    }

    fn dispose_resource_stacks_sync(
        &mut self,
        resources: Vec<BindingResourceStack>,
        mut completion: Completion,
    ) -> Result<Completion> {
        for resource in resources.into_iter().rev() {
            match resource {
                BindingResourceStack::Sync(stack) => {
                    completion = self.dispose_disposable_stack_completion(&stack, completion)?;
                }
                BindingResourceStack::Async(_) => {
                    return Err(Error::runtime(
                        "asynchronous resource reached synchronous scope disposal",
                    ));
                }
            }
        }
        Ok(completion)
    }

    fn begin_dispose_resource_stacks(
        &mut self,
        resources: Vec<BindingResourceStack>,
        completion: Completion,
    ) -> Result<ScopeDisposal> {
        if !resources
            .iter()
            .any(|resource| matches!(resource, BindingResourceStack::Async(_)))
        {
            return self
                .dispose_resource_stacks_sync(resources, completion)
                .map(ScopeDisposal::Complete);
        }
        let (result_promise, promise_object) = self.create_pending_promise()?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            core::iter::once(&promise_object),
        )?;
        let continuation = ResourceScopeContinuation::new(result_promise, resources, &completion);
        self.continue_resource_scope_disposal(continuation, None)?;
        Ok(ScopeDisposal::Await(result_promise))
    }

    pub(in crate::runtime) fn resume_resource_scope_disposal(
        &mut self,
        continuation: ResourceScopeContinuation,
        resume: Completion,
    ) -> Result<()> {
        self.continue_resource_scope_disposal(continuation, Some(resume))
    }

    fn continue_resource_scope_disposal(
        &mut self,
        mut continuation: ResourceScopeContinuation,
        resume: Option<Completion>,
    ) -> Result<()> {
        if let Some(Completion::Throw(reason)) = resume {
            self.record_resource_scope_error(&mut continuation, reason)?;
        }
        while let Some(resource) = continuation.resources.pop() {
            match resource {
                BindingResourceStack::Sync(stack) => {
                    let completion = continuation
                        .thrown
                        .take()
                        .map_or_else(|| Completion::Normal(Value::Undefined), Completion::Throw);
                    let completion =
                        self.dispose_disposable_stack_completion(&stack, completion)?;
                    continuation.thrown = match completion {
                        Completion::Throw(reason) => Some(reason),
                        _ => None,
                    };
                }
                BindingResourceStack::Async(stack) => {
                    let promise = self.eval_async_disposable_stack_function(
                        AsyncDisposableStackFunctionKind::DisposeAsync,
                        RuntimeCallArgs::values(&[]),
                        &stack,
                    )?;
                    let awaited = self.promise_resolve_for_await(promise)?;
                    self.add_promise_reaction(
                        awaited,
                        PromiseReaction::awaiting_resource_scope(continuation),
                    )?;
                    return Ok(());
                }
            }
        }
        if let Some(reason) = continuation.thrown {
            self.reject_promise(continuation.result_promise, reason)
        } else {
            self.resolve_promise(continuation.result_promise, Value::Undefined)
        }
    }

    fn record_resource_scope_error(
        &mut self,
        continuation: &mut ResourceScopeContinuation,
        error: Value,
    ) -> Result<()> {
        continuation.thrown = Some(if let Some(suppressed) = continuation.thrown.take() {
            self.create_suppressed_error(error, suppressed)?
        } else {
            error
        });
        Ok(())
    }
}
