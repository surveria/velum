use crate::{error::Result, value::Value};

use super::{Context, binding::scope::BindingScope, object::PropertyKey, promise::PromiseId};

const ROOT_KIND_COUNT: usize = 15;

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
            Self::RuntimeAnchor => 10,
            Self::RetainedHandle => 11,
            Self::TransientOperand => 12,
            Self::TransientCall => 13,
            Self::TransientTemporary => 14,
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
        visit_scope(&self.realm.globals, VmRootKind::GlobalBinding, visitor)?;
        visit_scope(
            &self.realm.builtin_globals,
            VmRootKind::BuiltinBinding,
            visitor,
        )?;
        for scope in &self.locals {
            visit_scope(scope, VmRootKind::LocalBinding, visitor)?;
        }
        for module in &self.modules {
            visit_scope(module.scope(), VmRootKind::ModuleBinding, visitor)?;
            visitor.visit_value(VmRootKind::ModuleBinding, module.namespace())?;
        }
        for frame in &self.activation_frames {
            if let Some(environments) = frame.with_environments() {
                for object in environments {
                    visitor.visit_value(VmRootKind::CapturedBinding, object)?;
                }
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
        if let Some(id) = self.realm.global_object {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.promise_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.generator_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.generator_function_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.async_iterator_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.async_generator_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.realm.async_generator_function_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
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
        for id in self.realm.native_function_registry.ids() {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::NativeFunction(id))?;
        }
        self.objects.visit_direct_roots(visitor)?;
        for job in &self.promise_jobs {
            job.visit_direct_roots(visitor)?;
        }
        self.retained_values.visit(visitor)?;
        self.transient_roots.visit(visitor)?;
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
