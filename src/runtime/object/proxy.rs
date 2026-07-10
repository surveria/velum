use crate::{
    error::Result,
    runtime::trace::{StrongEdgeReference, StrongEdgeVisitor, VmObjectEdgeKind},
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

/// Internal slots of a Proxy exotic object: the wrapped target and the handler
/// that supplies traps. A revoked proxy clears both references and rejects
/// every internal method with a `TypeError`.
#[derive(Debug, Clone)]
pub struct ProxyValue {
    target: Value,
    handler: Value,
    callable: bool,
    constructable: bool,
    revoked: bool,
}

impl ProxyValue {
    const fn new(target: Value, handler: Value, callable: bool, constructable: bool) -> Self {
        Self {
            target,
            handler,
            callable,
            constructable,
            revoked: false,
        }
    }

    pub(in crate::runtime) const fn callable(&self) -> bool {
        self.callable
    }

    pub(in crate::runtime) const fn constructable(&self) -> bool {
        self.constructable
    }

    pub(in crate::runtime) const fn target(&self) -> Option<&Value> {
        if self.revoked {
            return None;
        }
        Some(&self.target)
    }

    pub(in crate::runtime) const fn handler(&self) -> Option<&Value> {
        if self.revoked {
            return None;
        }
        Some(&self.handler)
    }

    pub(in crate::runtime::object) fn visit_strong_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit(
            VmObjectEdgeKind::InternalSlot,
            StrongEdgeReference::Value(&self.target),
        )?;
        visitor.visit(
            VmObjectEdgeKind::InternalSlot,
            StrongEdgeReference::Value(&self.handler),
        )
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn create_proxy_object(
        &mut self,
        target: Value,
        handler: Value,
        callable: bool,
        constructable: bool,
        max_objects: usize,
    ) -> Result<Value> {
        let object = Object::proxy(ProxyValue::new(target, handler, callable, constructable));
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(in crate::runtime) fn proxy_value(&self, id: ObjectId) -> Result<Option<&ProxyValue>> {
        Ok(self.object(id)?.proxy_value.as_ref())
    }

    pub(in crate::runtime) fn is_proxy(&self, id: ObjectId) -> bool {
        self.object(id)
            .is_ok_and(|object| object.proxy_value.is_some())
    }

    pub(in crate::runtime) fn proxy_callability(&self, id: ObjectId) -> Result<bool> {
        Ok(self.proxy_value(id)?.is_some_and(ProxyValue::callable))
    }

    pub(in crate::runtime) fn proxy_constructability(&self, id: ObjectId) -> Result<bool> {
        Ok(self.proxy_value(id)?.is_some_and(ProxyValue::constructable))
    }

    pub(in crate::runtime) fn revoke_proxy(&mut self, id: ObjectId) -> Result<()> {
        if let Some(proxy) = self.object_mut(id)?.proxy_value.as_mut() {
            proxy.revoked = true;
            proxy.target = Value::Null;
            proxy.handler = Value::Null;
        }
        Ok(())
    }
}

impl Object {
    pub(super) fn proxy(value: ProxyValue) -> Self {
        let mut object = Self::ordinary();
        object.proxy_value = Some(value);
        object
    }
}
