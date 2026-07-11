use crate::{
    error::Result,
    runtime::{
        collections::{CollectionId, CollectionIteratorId},
        object::PropertyKey,
        promise::PromiseId,
    },
    storage::{string_heap::JsString, symbol::JsSymbol},
    value::{BoundFunctionId, FunctionId, ObjectId, Value},
};

use super::{Context, Function, FunctionNewTarget};

const CALLABLE_EDGE_KIND_COUNT: usize = 6;
const OBJECT_EDGE_KIND_COUNT: usize = 3;

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

/// Strong-reference slot categories currently owned by ordinary object
/// storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VmObjectEdgeKind {
    Property,
    Prototype,
    InternalSlot,
}

impl VmObjectEdgeKind {
    const ALL: [Self; OBJECT_EDGE_KIND_COUNT] =
        [Self::Property, Self::Prototype, Self::InternalSlot];

    /// Returns every object edge category in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    const fn index(self) -> usize {
        match self {
            Self::Property => 0,
            Self::Prototype => 1,
            Self::InternalSlot => 2,
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

/// Counted view of strong-reference slots stored in the ordinary object
/// arena. Context side-table associations are intentionally excluded until
/// AS-05b1b3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmObjectEdgeSnapshot {
    counts: [usize; OBJECT_EDGE_KIND_COUNT],
    total: usize,
}

impl VmObjectEdgeSnapshot {
    fn capture(context: &Context) -> Result<Self> {
        let mut counter = ObjectEdgeCounter::new();
        context.visit_object_edges(&mut counter)?;
        Ok(Self {
            counts: counter.counts,
            total: counter.total,
        })
    }

    /// Returns the number of physical reference slots in one category.
    #[must_use]
    pub fn count(self, kind: VmObjectEdgeKind) -> usize {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the total number of object-arena reference slots.
    #[must_use]
    pub const fn total(self) -> usize {
        self.total
    }

    /// Returns whether the object arena currently contains zero strong edges.
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
    String(&'value JsString),
    Symbol(&'value JsSymbol),
    PromiseAssociation {
        object: ObjectId,
        promise: PromiseId,
    },
    CollectionAssociation {
        object: ObjectId,
        collection: CollectionId,
    },
}

pub(in crate::runtime) trait StrongEdgeVisitor<Kind> {
    fn visit(&mut self, kind: Kind, reference: StrongEdgeReference<'_>) -> Result<()>;
}

/// Typed target used only by weak-key and ephemeron traversal.
#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) enum WeakEdgeReference<'value> {
    Value(&'value Value),
}

pub(in crate::runtime) trait WeakEdgeVisitor<Kind> {
    fn visit_weak(&mut self, kind: Kind, reference: WeakEdgeReference<'_>) -> Result<()>;

    fn visit_ephemeron(
        &mut self,
        kind: Kind,
        key: WeakEdgeReference<'_>,
        value: WeakEdgeReference<'_>,
    ) -> Result<()>;
}

struct CallableEdgeCounter {
    counts: [usize; CALLABLE_EDGE_KIND_COUNT],
    total: usize,
}

struct ObjectEdgeCounter {
    counts: [usize; OBJECT_EDGE_KIND_COUNT],
    total: usize,
}

impl ObjectEdgeCounter {
    const fn new() -> Self {
        Self {
            counts: [0; OBJECT_EDGE_KIND_COUNT],
            total: 0,
        }
    }
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

impl StrongEdgeVisitor<VmObjectEdgeKind> for ObjectEdgeCounter {
    fn visit(&mut self, kind: VmObjectEdgeKind, reference: StrongEdgeReference<'_>) -> Result<()> {
        consume_reference(&reference);
        let count = self
            .counts
            .get_mut(kind.index())
            .ok_or_else(|| crate::Error::runtime("object edge kind index is not defined"))?;
        *count = count
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("object edge category count overflowed"))?;
        self.total = self
            .total
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("object edge count overflowed"))?;
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

    /// Counts strong-reference slots stored directly in the ordinary object
    /// arena. Promise and collection side-table associations are excluded.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range.
    pub fn object_edge_snapshot(&self) -> Result<VmObjectEdgeSnapshot> {
        VmObjectEdgeSnapshot::capture(self)
    }

    pub(in crate::runtime) fn visit_object_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        self.objects.visit_strong_edges(visitor)
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
    pub(in crate::runtime) fn visit_strong_edges<V: StrongEdgeVisitor<VmCallableEdgeKind>>(
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

pub(in crate::runtime) const fn consume_reference(reference: &StrongEdgeReference<'_>) {
    match reference {
        StrongEdgeReference::Value(_value) => {}
        StrongEdgeReference::Function(_id) => {}
        StrongEdgeReference::Object(_id) => {}
        StrongEdgeReference::Promise(_id) => {}
        StrongEdgeReference::BoundFunction(_id) => {}
        StrongEdgeReference::CollectionIterator(_id) => {}
        StrongEdgeReference::PropertyKey(_key) => {}
        StrongEdgeReference::String(_string) => {}
        StrongEdgeReference::Symbol(_symbol) => {}
        StrongEdgeReference::PromiseAssociation {
            object: _object,
            promise: _promise,
        } => {}
        StrongEdgeReference::CollectionAssociation {
            object: _object,
            collection: _collection,
        } => {}
    }
}

pub(in crate::runtime) const fn consume_weak_reference(reference: WeakEdgeReference<'_>) {
    match reference {
        WeakEdgeReference::Value(_value) => {}
    }
}
