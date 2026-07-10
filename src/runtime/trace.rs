use crate::{
    error::Result,
    runtime::{collections::CollectionIteratorId, object::PropertyKey, promise::PromiseId},
    value::{BoundFunctionId, FunctionId, ObjectId, Value},
};

use super::{Context, Function, FunctionNewTarget};

const CALLABLE_EDGE_KIND_COUNT: usize = 6;

/// Strong-reference slot categories currently owned by callable stores.
///
/// This enum is non-exhaustive because later AS-05b1b slices add object,
/// Promise, collection, and iterator categories to the shared trace contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VmCallableEdgeKind {
    JavaScriptFunctionUpvalue,
    JavaScriptFunctionProperty,
    JavaScriptFunctionInternal,
    NativeFunctionProperty,
    NativeFunctionInternal,
    BoundFunctionInternal,
}

impl VmCallableEdgeKind {
    const ALL: [Self; CALLABLE_EDGE_KIND_COUNT] = [
        Self::JavaScriptFunctionUpvalue,
        Self::JavaScriptFunctionProperty,
        Self::JavaScriptFunctionInternal,
        Self::NativeFunctionProperty,
        Self::NativeFunctionInternal,
        Self::BoundFunctionInternal,
    ];

    /// Returns every callable edge category in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    const fn index(self) -> usize {
        match self {
            Self::JavaScriptFunctionUpvalue => 0,
            Self::JavaScriptFunctionProperty => 1,
            Self::JavaScriptFunctionInternal => 2,
            Self::NativeFunctionProperty => 3,
            Self::NativeFunctionInternal => 4,
            Self::BoundFunctionInternal => 5,
        }
    }
}

/// Counted view of strong-reference slots stored in callable arenas.
///
/// Counts describe physical reference slots, not unique or reachable heap
/// nodes. The future marker starts from direct roots, follows these slots, and
/// owns identity deduplication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmCallableEdgeSnapshot {
    counts: [usize; CALLABLE_EDGE_KIND_COUNT],
    total: usize,
}

impl VmCallableEdgeSnapshot {
    fn capture(context: &Context) -> Result<Self> {
        let mut counter = CallableEdgeCounter::new();
        context.visit_callable_edges(&mut counter)?;
        Ok(Self {
            counts: counter.counts,
            total: counter.total,
        })
    }

    /// Returns the number of physical reference slots in one category.
    #[must_use]
    pub fn count(self, kind: VmCallableEdgeKind) -> usize {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the total number of callable-store reference slots.
    #[must_use]
    pub const fn total(self) -> usize {
        self.total
    }

    /// Returns whether every callable store currently contains zero edges.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.total == 0
    }
}

/// Typed target of one strong VM edge.
///
/// The payload remains internal so diagnostic snapshots expose counts without
/// leaking forgeable arena ids. Later markers can distinguish value, arena,
/// association, and property-key targets without encoding ids as integers.
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) enum StrongEdgeReference<'value> {
    Value(&'value Value),
    Function(FunctionId),
    Object(ObjectId),
    Promise(PromiseId),
    BoundFunction(BoundFunctionId),
    CollectionIterator(CollectionIteratorId),
    PropertyKey(PropertyKey),
}

pub(in crate::runtime) trait StrongEdgeVisitor<Kind> {
    fn visit(&mut self, kind: Kind, reference: StrongEdgeReference<'_>) -> Result<()>;
}

struct CallableEdgeCounter {
    counts: [usize; CALLABLE_EDGE_KIND_COUNT],
    total: usize,
}

impl CallableEdgeCounter {
    const fn new() -> Self {
        Self {
            counts: [0; CALLABLE_EDGE_KIND_COUNT],
            total: 0,
        }
    }
}

impl StrongEdgeVisitor<VmCallableEdgeKind> for CallableEdgeCounter {
    fn visit(
        &mut self,
        kind: VmCallableEdgeKind,
        reference: StrongEdgeReference<'_>,
    ) -> Result<()> {
        consume_reference(&reference);
        let count = self
            .counts
            .get_mut(kind.index())
            .ok_or_else(|| crate::Error::runtime("callable edge kind index is not defined"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("callable edge category count overflowed"))?;
        self.total = self
            .total
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("callable edge count overflowed"))?;
        Ok(())
    }
}

impl Context {
    /// Counts strong-reference slots in JavaScript, native, and bound function
    /// stores. This does not include object or asynchronous arenas.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range.
    pub fn callable_edge_snapshot(&self) -> Result<VmCallableEdgeSnapshot> {
        VmCallableEdgeSnapshot::capture(self)
    }

    pub(in crate::runtime) fn visit_callable_edges<V: StrongEdgeVisitor<VmCallableEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for function in &self.functions {
            function.visit_strong_edges(visitor)?;
        }
        for function in &self.native_functions {
            function.visit_strong_edges(visitor)?;
        }
        for function in &self.bound_functions {
            function.visit_strong_edges(visitor)?;
        }
        Ok(())
    }
}

impl Function {
    fn visit_strong_edges<V: StrongEdgeVisitor<VmCallableEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for cell in self.upvalues.iter() {
            if let Some(result) = cell.with_initialized_value(|value| {
                visitor.visit(
                    VmCallableEdgeKind::JavaScriptFunctionUpvalue,
                    StrongEdgeReference::Value(value),
                )
            }) {
                result?;
            }
        }
        self.properties
            .visit_strong_edges(VmCallableEdgeKind::JavaScriptFunctionProperty, visitor)?;
        if let Some(binding) = &self.super_binding {
            if let Some(constructor) = &binding.constructor {
                visitor.visit(
                    VmCallableEdgeKind::JavaScriptFunctionInternal,
                    StrongEdgeReference::Value(constructor),
                )?;
            }
            visitor.visit(
                VmCallableEdgeKind::JavaScriptFunctionInternal,
                StrongEdgeReference::Value(&binding.home_prototype),
            )?;
            if let Some(constructor) = binding.own_constructor {
                visitor.visit(
                    VmCallableEdgeKind::JavaScriptFunctionInternal,
                    StrongEdgeReference::Function(constructor),
                )?;
            }
        }
        if let Some(parent) = &self.static_parent {
            visitor.visit(
                VmCallableEdgeKind::JavaScriptFunctionInternal,
                StrongEdgeReference::Value(parent),
            )?;
        }
        if let Some(fields) = &self.class_fields {
            for field in fields.iter() {
                visitor.visit(
                    VmCallableEdgeKind::JavaScriptFunctionInternal,
                    StrongEdgeReference::PropertyKey(field.key),
                )?;
            }
        }
        if let FunctionNewTarget::Lexical(value) = &self.new_target {
            visitor.visit(
                VmCallableEdgeKind::JavaScriptFunctionInternal,
                StrongEdgeReference::Value(value),
            )?;
        }
        Ok(())
    }
}

const fn consume_reference(reference: &StrongEdgeReference<'_>) {
    match reference {
        StrongEdgeReference::Value(_value) => {}
        StrongEdgeReference::Function(_id) => {}
        StrongEdgeReference::Object(_id) => {}
        StrongEdgeReference::Promise(_id) => {}
        StrongEdgeReference::BoundFunction(_id) => {}
        StrongEdgeReference::CollectionIterator(_id) => {}
        StrongEdgeReference::PropertyKey(_key) => {}
    }
}
