use crate::{error::Result, value::Value};

use super::{Context, binding::scope::BindingScope, object::PropertyKey, promise::PromiseId};

const ROOT_KIND_COUNT: usize = 9;

/// Direct VM root categories that exist independently of heap trace edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmRootKind {
    GlobalBinding,
    BuiltinBinding,
    LocalBinding,
    CapturedBinding,
    ActiveThis,
    ActiveNewTarget,
    ActiveSuper,
    QueuedJob,
    RuntimeAnchor,
}

impl VmRootKind {
    const ALL: [Self; ROOT_KIND_COUNT] = [
        Self::GlobalBinding,
        Self::BuiltinBinding,
        Self::LocalBinding,
        Self::CapturedBinding,
        Self::ActiveThis,
        Self::ActiveNewTarget,
        Self::ActiveSuper,
        Self::QueuedJob,
        Self::RuntimeAnchor,
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
            Self::CapturedBinding => 3,
            Self::ActiveThis => 4,
            Self::ActiveNewTarget => 5,
            Self::ActiveSuper => 6,
            Self::QueuedJob => 7,
            Self::RuntimeAnchor => 8,
        }
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
        visit_scope(&self.globals, VmRootKind::GlobalBinding, visitor)?;
        visit_scope(&self.builtin_globals, VmRootKind::BuiltinBinding, visitor)?;
        for scope in &self.locals {
            visit_scope(scope, VmRootKind::LocalBinding, visitor)?;
        }
        for frame in &self.upvalue_frames {
            for cell in frame.iter() {
                visit_cell(cell, VmRootKind::CapturedBinding, visitor)?;
            }
        }
        for value in &self.this_values {
            visitor.visit_value(VmRootKind::ActiveThis, value)?;
        }
        for value in &self.new_target_values {
            visitor.visit_value(VmRootKind::ActiveNewTarget, value)?;
        }
        for frame in self.super_frames.iter().flatten() {
            if let Some(constructor) = &frame.constructor {
                visitor.visit_value(VmRootKind::ActiveSuper, constructor)?;
            }
            visitor.visit_value(VmRootKind::ActiveSuper, &frame.home_prototype)?;
        }
        if let Some(id) = self.global_object {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.promise_prototype {
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
        for id in self.native_function_registry.ids() {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::NativeFunction(id))?;
        }
        self.objects.visit_direct_roots(visitor)?;
        for job in &self.promise_jobs {
            job.visit_direct_roots(visitor)?;
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
