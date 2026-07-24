use crate::{error::Result, value::Value};

use super::{Context, binding::scope::BindingScope, object::PropertyKey, promise::PromiseId};

const ROOT_KIND_COUNT: usize = 16;

/// Direct VM root categories that exist independently of heap trace edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmRootKind {
    GlobalBinding,
    BuiltinBinding,
    LocalBinding,
    ModuleBinding,
    CapturedBinding,
    ActiveThis,
    ActiveNewTarget,
    ActiveSuper,
    BytecodeFrame,
    QueuedJob,
    HostFuture,
    RuntimeAnchor,
    RetainedHandle,
    TransientOperand,
    TransientCall,
    TransientTemporary,
}

impl VmRootKind {
    const ALL: [Self; ROOT_KIND_COUNT] = [
        Self::GlobalBinding,
        Self::BuiltinBinding,
        Self::LocalBinding,
        Self::ModuleBinding,
        Self::CapturedBinding,
        Self::ActiveThis,
        Self::ActiveNewTarget,
        Self::ActiveSuper,
        Self::BytecodeFrame,
        Self::QueuedJob,
        Self::HostFuture,
        Self::RuntimeAnchor,
        Self::RetainedHandle,
        Self::TransientOperand,
        Self::TransientCall,
        Self::TransientTemporary,
    ];

    /// Returns every direct-root category in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    const fn index(self) -> usize {
        match self {
            Self::GlobalBinding => 0,
            Self::BuiltinBinding => 1,
            Self::LocalBinding => 2,
            Self::ModuleBinding => 3,
            Self::CapturedBinding => 4,
            Self::ActiveThis => 5,
            Self::ActiveNewTarget => 6,
            Self::ActiveSuper => 7,
            Self::BytecodeFrame => 8,
            Self::QueuedJob => 9,
            Self::HostFuture => 10,
            Self::RuntimeAnchor => 11,
            Self::RetainedHandle => 12,
            Self::TransientOperand => 13,
            Self::TransientCall => 14,
            Self::TransientTemporary => 15,
        }
    }

    pub(in crate::runtime) const fn is_transient(self) -> bool {
        matches!(
            self,
            Self::TransientOperand | Self::TransientCall | Self::TransientTemporary
        )
    }
}

/// Counted view of the direct root references currently owned by a VM.
///
/// Counts are references, not unique values. The same binding cell may be
/// reachable from an active scope and a captured frame; a future marker is
/// responsible for deduplicating the heap identities reached from them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmRootSnapshot {
    counts: [usize; ROOT_KIND_COUNT],
    total: usize,
}

impl VmRootSnapshot {
    fn capture(context: &Context) -> Result<Self> {
        let mut counter = RootCounter::new();
        context.visit_direct_roots(&mut counter)?;
        Ok(Self {
            counts: counter.counts,
            total: counter.total,
        })
    }

    /// Returns the number of direct root references in one category.
    #[must_use]
    pub fn count(self, kind: VmRootKind) -> usize {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the total number of direct root references.
    #[must_use]
    pub const fn total(self) -> usize {
        self.total
    }

    /// Returns whether the VM currently has no direct roots.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.total == 0
    }
}

pub(in crate::runtime) trait DirectRootVisitor {
    fn visit_value(&mut self, kind: VmRootKind, value: &Value) -> Result<()>;

    fn visit_promise(&mut self, kind: VmRootKind, promise: PromiseId) -> Result<()>;

    fn visit_property_key(&mut self, kind: VmRootKind, key: PropertyKey) -> Result<()>;
}

struct RootCounter {
    counts: [usize; ROOT_KIND_COUNT],
    total: usize,
}

impl RootCounter {
    const fn new() -> Self {
        Self {
            counts: [0; ROOT_KIND_COUNT],
            total: 0,
        }
    }
}

impl DirectRootVisitor for RootCounter {
    fn visit_value(&mut self, kind: VmRootKind, _value: &Value) -> Result<()> {
        self.record(kind)
    }

    fn visit_promise(&mut self, kind: VmRootKind, _promise: PromiseId) -> Result<()> {
        self.record(kind)
    }

    fn visit_property_key(&mut self, kind: VmRootKind, _key: PropertyKey) -> Result<()> {
        self.record(kind)
    }
}

impl RootCounter {
    fn record(&mut self, kind: VmRootKind) -> Result<()> {
        let count = self
            .counts
            .get_mut(kind.index())
            .ok_or_else(|| crate::Error::runtime("root kind index is not defined"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("root category count overflowed"))?;
        self.total = self
            .total
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("root reference count overflowed"))?;
        Ok(())
    }
}

impl Context {
    /// Counts every direct root reference currently stored in this Context.
    ///
    /// # Errors
    /// Fails if a root-reference counter exceeds the supported range.
    pub fn root_snapshot(&self) -> Result<VmRootSnapshot> {
        VmRootSnapshot::capture(self)
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for realm in self.realm_states() {
            visit_scope(&realm.globals, VmRootKind::GlobalBinding, visitor)?;
            visit_scope(&realm.builtin_globals, VmRootKind::BuiltinBinding, visitor)?;
        }
        for scope in &self.locals {
            visit_scope(scope, VmRootKind::LocalBinding, visitor)?;
        }
        self.visit_module_roots(visitor)?;
        if let Some(import_meta) = &self.active_import_meta {
            visitor.visit_value(VmRootKind::ModuleBinding, import_meta)?;
        }
        self.visit_activation_roots(visitor)?;
        for promise in &self.active_async_function_promises {
            visitor.visit_promise(VmRootKind::BytecodeFrame, *promise)?;
        }
        self.visit_runtime_anchor_roots(visitor)?;
        for cache in &self.static_name_atom_caches {
            cache.visit_template_objects(|value| {
                visitor.visit_value(VmRootKind::BytecodeFrame, value)
            })?;
        }
        self.objects.visit_direct_roots(visitor)?;
        for job in &self.promise_jobs {
            job.visit_direct_roots(visitor)?;
        }
        self.visit_host_future_roots(visitor)?;
        self.retained_values.visit(visitor)?;
        self.transient_roots.visit(visitor)?;
        Ok(())
    }

    fn visit_activation_roots<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        for frame in &self.activation_frames {
            if let Some(environments) = frame.dynamic_environments() {
                for environment in environments {
                    environment.for_each_value(|value| {
                        visitor.visit_value(VmRootKind::CapturedBinding, value)
                    })?;
                }
            }
            if let Some(arguments) = frame.legacy_arguments() {
                arguments
                    .for_each_value(|value| visitor.visit_value(VmRootKind::LocalBinding, value))?;
            }
            if let Some(upvalues) = frame.upvalues() {
                for cell in upvalues.iter() {
                    visit_cell(cell, VmRootKind::CapturedBinding, visitor)?;
                }
            }
            if let Some(value) = frame.this_value() {
                visitor.visit_value(VmRootKind::ActiveThis, value)?;
            }
            if let Some(value) = frame.new_target() {
                visitor.visit_value(VmRootKind::ActiveNewTarget, value)?;
            }
            if let Some(super_binding) = frame.super_binding() {
                if let Some(constructor) = &super_binding.constructor {
                    visitor.visit_value(VmRootKind::ActiveSuper, constructor)?;
                }
                visitor.visit_value(VmRootKind::ActiveSuper, &super_binding.home_object)?;
                if let Some(this_value) = super_binding.this_value.borrow().as_ref() {
                    visitor.visit_value(VmRootKind::ActiveSuper, this_value)?;
                }
            }
            if let Some(continuation) = frame.continuation() {
                if let Some(function) = continuation.function_id() {
                    visitor.visit_value(VmRootKind::BytecodeFrame, &Value::Function(function))?;
                }
                for value in continuation.root_values() {
                    visitor.visit_value(VmRootKind::BytecodeFrame, value)?;
                }
            }
        }
        Ok(())
    }

    fn visit_runtime_anchor_roots<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        for realm in self.realm_states() {
            for id in realm.anchor_objects() {
                visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
            }
            for value in realm.anchor_values() {
                visitor.visit_value(VmRootKind::RuntimeAnchor, value)?;
            }
        }
        if let Some(symbol) = self.iterator_symbol {
            visitor.visit_property_key(VmRootKind::RuntimeAnchor, PropertyKey::symbol(symbol))?;
        }
        for key in self.well_known_properties.keys() {
            visitor.visit_property_key(VmRootKind::RuntimeAnchor, key)?;
        }
        if let Some(keys) = self.descriptor_property_keys {
            for key in keys.keys() {
                visitor.visit_property_key(VmRootKind::RuntimeAnchor, key)?;
            }
        }
        for id in self.symbols.registered_ids() {
            visitor.visit_value(
                VmRootKind::RuntimeAnchor,
                &Value::Symbol(self.symbols.get(id)?.clone()),
            )?;
        }
        for (_, id) in &self.well_known_symbols {
            visitor.visit_value(
                VmRootKind::RuntimeAnchor,
                &Value::Symbol(self.symbols.get(*id)?.clone()),
            )?;
        }
        for realm in self.realm_states() {
            for id in realm.native_function_ids() {
                visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::NativeFunction(id))?;
            }
            for id in realm.host_function_ids() {
                visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::HostFunction(id))?;
            }
        }
        Ok(())
    }

    fn visit_module_roots<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        for module in &self.modules {
            if let Some(scope) = module.scope() {
                visit_scope(scope, VmRootKind::ModuleBinding, visitor)?;
            }
            visitor.visit_value(VmRootKind::ModuleBinding, module.namespace())?;
            visitor.visit_value(VmRootKind::ModuleBinding, module.deferred_namespace())?;
            if let Some(source) = module.module_source() {
                visitor.visit_value(VmRootKind::ModuleBinding, source)?;
            }
            if let Some(import_meta) = module.import_meta() {
                visitor.visit_value(VmRootKind::ModuleBinding, import_meta)?;
            }
            if let Some(error) = module.evaluation_error_value() {
                visitor.visit_value(VmRootKind::ModuleBinding, error)?;
            }
            if let Some(value) = module.evaluation_value() {
                visitor.visit_value(VmRootKind::ModuleBinding, value)?;
            }
            if let Some(promise) = module.evaluation_promise() {
                visitor.visit_promise(VmRootKind::ModuleBinding, promise)?;
            }
            if let Some(execution) = module.execution() {
                execution.visit_direct_roots(visitor)?;
            }
        }
        Ok(())
    }
}

fn visit_scope<V: DirectRootVisitor>(
    scope: &BindingScope,
    kind: VmRootKind,
    visitor: &mut V,
) -> Result<()> {
    for cell in scope.cells() {
        visit_cell(cell, kind, visitor)?;
    }
    for stack in scope.resource_stacks() {
        visitor.visit_value(kind, stack.value())?;
    }
    Ok(())
}

fn visit_cell<V: DirectRootVisitor>(
    cell: &super::binding::scope::BindingCell,
    kind: VmRootKind,
    visitor: &mut V,
) -> Result<()> {
    if let Some(result) = cell.with_initialized_value(|value| visitor.visit_value(kind, value)) {
        result?;
    }
    Ok(())
}
