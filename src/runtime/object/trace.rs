use crate::{
    error::Result,
    runtime::{
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor, VmObjectEdgeKind},
    },
    value::Value,
};

use super::{Object, ObjectHeap, ObjectPrimitiveValue};

impl ObjectHeap {
    pub(in crate::runtime) fn visit_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for object in &self.objects {
            object.visit_strong_edges(visitor)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn visit_object_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        id: crate::value::ObjectId,
        visitor: &mut V,
    ) -> Result<()> {
        self.object(id)?.visit_strong_edges(visitor)
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        if let Some(id) = self.object_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        if let Some(id) = self.array_prototype {
            visitor.visit_value(VmRootKind::RuntimeAnchor, &Value::Object(id))?;
        }
        for key in self.shapes.property_keys() {
            visitor.visit_property_key(VmRootKind::RuntimeAnchor, key)?;
        }
        Ok(())
    }
}

impl Object {
    fn visit_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for entry in &self.named_properties {
            visitor.visit(
                VmObjectEdgeKind::Property,
                StrongEdgeReference::PropertyKey(entry.key()),
            )?;
            entry
                .property()
                .visit_strong_edges(VmObjectEdgeKind::Property, visitor)?;
        }
        self.array_storage.visit_strong_edges(visitor)?;
        if let Some(prototype) = self.prototype {
            visitor.visit(
                VmObjectEdgeKind::Prototype,
                StrongEdgeReference::Object(prototype),
            )?;
        }
        if let Some(string) = &self.string_value {
            visitor.visit(
                VmObjectEdgeKind::InternalSlot,
                StrongEdgeReference::String(string),
            )?;
        }
        if let Some(ObjectPrimitiveValue::Symbol(symbol)) = &self.primitive_value {
            visitor.visit(
                VmObjectEdgeKind::InternalSlot,
                StrongEdgeReference::Symbol(symbol),
            )?;
        }
        if let Some(proxy) = &self.proxy_value {
            proxy.visit_strong_edges(visitor)?;
        }
        if let Some(view) = &self.uint8_array {
            visitor.visit(
                VmObjectEdgeKind::InternalSlot,
                StrongEdgeReference::Object(view.buffer_object()),
            )?;
        }
        Ok(())
    }
}
