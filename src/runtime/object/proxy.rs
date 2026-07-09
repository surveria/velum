use crate::{
    error::Result,
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
    revoked: bool,
}

impl ProxyValue {
    const fn new(target: Value, handler: Value) -> Self {
        Self {
            target,
            handler,
            revoked: false,
        }
    }

    pub(in crate::runtime) fn target(&self) -> Option<&Value> {
        if self.revoked {
            return None;
        }
        Some(&self.target)
    }

    pub(in crate::runtime) fn handler(&self) -> Option<&Value> {
        if self.revoked {
            return None;
        }
        Some(&self.handler)
    }

    pub(in crate::runtime) const fn is_revoked(&self) -> bool {
        self.revoked
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn create_proxy_object(
        &mut self,
        target: Value,
        handler: Value,
        max_objects: usize,
    ) -> Result<Value> {
        let object = Object::proxy(ProxyValue::new(target, handler));
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(in crate::runtime) fn proxy_value(&self, id: ObjectId) -> Result<Option<&ProxyValue>> {
        Ok(self.object(id)?.proxy_value.as_ref())
    }

    pub(in crate::runtime) fn is_proxy(&self, id: ObjectId) -> bool {
        self.object(id)
            .map(|object| object.proxy_value.is_some())
            .unwrap_or(false)
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
