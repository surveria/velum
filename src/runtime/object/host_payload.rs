#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::{any::Any, fmt};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        arena::SlotArena,
        storage_ledger::VmStorageLedger,
        trace::{StrongEdgeReference, StrongEdgeVisitor, VmObjectEdgeKind},
    },
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

const HOST_OBJECT_TYPE_ERROR: &str = "value is not a typed host object";
const HOST_PAYLOAD_TYPE_ERROR: &str = "typed host object payload type does not match";
const HOST_PAYLOAD_ASSOCIATION_ERROR: &str = "host payload object association is not defined";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostPayloadId(usize);

impl HostPayloadId {
    const fn index(self) -> usize {
        self.0
    }
}

struct HostPayloadEntry {
    payload: Box<dyn Any>,
    logical_bytes: usize,
    wrapper_count: usize,
}

impl fmt::Debug for HostPayloadEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostPayloadEntry")
            .field("payload_type", &self.payload.as_ref().type_id())
            .field("logical_bytes", &self.logical_bytes)
            .field("wrapper_count", &self.wrapper_count)
            .finish()
    }
}

#[derive(Debug)]
struct HostPayloadAssociation {
    payload: HostPayloadId,
    traced_values: Vec<Value>,
}

#[derive(Debug)]
pub(super) struct HostPayloadRegistry {
    payloads: SlotArena<HostPayloadEntry>,
    object_payloads: Vec<Option<HostPayloadAssociation>>,
    storage_ledger: VmStorageLedger,
}

impl HostPayloadRegistry {
    pub(super) const fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            payloads: SlotArena::new(),
            object_payloads: Vec::new(),
            storage_ledger,
        }
    }

    fn prepare_association(&mut self, object_index: usize) -> Result<()> {
        if object_index >= self.object_payloads.len() {
            let required_len = object_index
                .checked_add(1)
                .ok_or_else(|| Error::limit("host payload association index overflowed"))?;
            let additional = required_len
                .checked_sub(self.object_payloads.len())
                .ok_or_else(|| Error::runtime(HOST_PAYLOAD_ASSOCIATION_ERROR))?;
            self.object_payloads
                .try_reserve(additional)
                .map_err(|_| Error::limit("host payload association capacity exceeded"))?;
            return Ok(());
        }
        if self
            .object_payloads
            .get(object_index)
            .is_some_and(Option::is_some)
        {
            return Err(Error::runtime("object already has a typed host payload"));
        }
        Ok(())
    }

    fn install_association(
        &mut self,
        object_index: usize,
        association: HostPayloadAssociation,
    ) -> Result<()> {
        while object_index > self.object_payloads.len() {
            self.object_payloads.push(None);
        }
        if object_index == self.object_payloads.len() {
            self.object_payloads.push(Some(association));
            return Ok(());
        }
        let Some(slot) = self.object_payloads.get_mut(object_index) else {
            return Err(Error::runtime(HOST_PAYLOAD_ASSOCIATION_ERROR));
        };
        if slot.is_some() {
            return Err(Error::runtime(
                "object host payload association was occupied",
            ));
        }
        *slot = Some(association);
        Ok(())
    }

    fn attach_new(
        &mut self,
        object_index: usize,
        payload: Box<dyn Any>,
        traced_values: Vec<Value>,
        logical_bytes: usize,
    ) -> Result<()> {
        self.prepare_association(object_index)?;
        self.payloads.reserve_insert()?;
        self.payloads.reserve_removals(1)?;
        let payload_reservation =
            self.storage_ledger
                .reserve(VmStorageKind::HostPayload, 1, logical_bytes)?;
        let instance_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::HostInstance, 1)?;
        payload_reservation.commit()?;
        if let Err(error) = instance_reservation.commit() {
            self.storage_ledger
                .release(VmStorageKind::HostPayload, 1, logical_bytes)?;
            return Err(error);
        }

        let entry = HostPayloadEntry {
            payload,
            logical_bytes,
            wrapper_count: 1,
        };
        let id = match self.payloads.insert(entry) {
            Ok(index) => HostPayloadId(index),
            Err(error) => {
                self.storage_ledger
                    .release_count(VmStorageKind::HostInstance, 1)?;
                self.storage_ledger
                    .release(VmStorageKind::HostPayload, 1, logical_bytes)?;
                return Err(error);
            }
        };
        let association = HostPayloadAssociation {
            payload: id,
            traced_values,
        };
        if let Err(error) = self.install_association(object_index, association) {
            let removed = self.payloads.remove_reserved(id.index())?;
            if removed.is_none() {
                return Err(Error::runtime("host payload rollback entry disappeared"));
            }
            self.storage_ledger
                .release_count(VmStorageKind::HostInstance, 1)?;
            self.storage_ledger
                .release(VmStorageKind::HostPayload, 1, logical_bytes)?;
            return Err(error);
        }
        Ok(())
    }

    fn attach_shared(&mut self, object_index: usize, source_index: usize) -> Result<()> {
        let payload_id = self.payload_id(source_index)?;
        let source_edges = self
            .object_payloads
            .get(source_index)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::runtime(HOST_PAYLOAD_ASSOCIATION_ERROR))?
            .traced_values
            .as_slice();
        let mut traced_values = Vec::new();
        traced_values
            .try_reserve(source_edges.len())
            .map_err(|_| Error::limit("host payload traced-value capacity exceeded"))?;
        traced_values.extend(source_edges.iter().cloned());
        let previous_wrapper_count = self
            .payloads
            .get(payload_id.index())
            .ok_or_else(|| Error::runtime(HOST_OBJECT_TYPE_ERROR))?
            .wrapper_count;
        let wrapper_count = previous_wrapper_count
            .checked_add(1)
            .ok_or_else(|| Error::limit("host payload wrapper count overflowed"))?;
        self.prepare_association(object_index)?;
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::HostInstance, 1)?;
        reservation.commit()?;
        let Some(entry) = self.payloads.get_mut(payload_id.index()) else {
            self.storage_ledger
                .release_count(VmStorageKind::HostInstance, 1)?;
            return Err(Error::runtime("shared host payload entry disappeared"));
        };
        entry.wrapper_count = wrapper_count;
        let association = HostPayloadAssociation {
            payload: payload_id,
            traced_values,
        };
        if let Err(error) = self.install_association(object_index, association) {
            let Some(entry) = self.payloads.get_mut(payload_id.index()) else {
                return Err(Error::runtime(
                    "shared host payload rollback entry disappeared",
                ));
            };
            entry.wrapper_count = previous_wrapper_count;
            self.storage_ledger
                .release_count(VmStorageKind::HostInstance, 1)?;
            return Err(error);
        }
        Ok(())
    }

    fn payload_id(&self, object_index: usize) -> Result<HostPayloadId> {
        self.object_payloads
            .get(object_index)
            .and_then(Option::as_ref)
            .map(|association| association.payload)
            .ok_or_else(|| Error::runtime(HOST_OBJECT_TYPE_ERROR))
    }

    fn payload<T: 'static>(&self, object_index: usize) -> Result<&T> {
        let id = self.payload_id(object_index)?;
        self.payloads
            .get(id.index())
            .and_then(|entry| entry.payload.downcast_ref::<T>())
            .ok_or_else(|| Error::runtime(HOST_PAYLOAD_TYPE_ERROR))
    }

    fn update_logical_bytes(&mut self, object_index: usize, logical_bytes: usize) -> Result<()> {
        let id = self.payload_id(object_index)?;
        let old_bytes = self
            .payloads
            .get(id.index())
            .ok_or_else(|| Error::runtime("host payload entry is not defined"))?
            .logical_bytes;
        let reservation = self.storage_ledger.reserve_replacement(
            VmStorageKind::HostPayload,
            0,
            old_bytes,
            0,
            logical_bytes,
        )?;
        reservation.commit()?;
        let Some(entry) = self.payloads.get_mut(id.index()) else {
            self.storage_ledger
                .reserve_replacement(VmStorageKind::HostPayload, 0, logical_bytes, 0, old_bytes)?
                .commit()?;
            return Err(Error::runtime("host payload entry disappeared"));
        };
        entry.logical_bytes = logical_bytes;
        Ok(())
    }

    fn detach_created(&mut self, object_index: usize) -> Result<()> {
        let Some(slot) = self.object_payloads.get_mut(object_index) else {
            return Err(Error::runtime(HOST_PAYLOAD_ASSOCIATION_ERROR));
        };
        let Some(association) = slot.take() else {
            return Err(Error::runtime(HOST_OBJECT_TYPE_ERROR));
        };
        let id = association.payload;
        let entry = self
            .payloads
            .get(id.index())
            .ok_or_else(|| Error::runtime("host payload rollback entry is missing"))?;
        let wrapper_count = entry
            .wrapper_count
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("host payload wrapper count underflowed"))?;
        let released_payload_bytes = (wrapper_count == 0).then_some(entry.logical_bytes);
        self.storage_ledger
            .release_count(VmStorageKind::HostInstance, 1)?;
        if let Some(payload_bytes) = released_payload_bytes {
            self.storage_ledger
                .release(VmStorageKind::HostPayload, 1, payload_bytes)?;
            let removed = self.payloads.remove_reserved(id.index())?;
            if removed.is_none() {
                return Err(Error::runtime("host payload rollback failed"));
            }
        } else {
            let Some(entry) = self.payloads.get_mut(id.index()) else {
                return Err(Error::runtime("shared host payload entry disappeared"));
            };
            entry.wrapper_count = wrapper_count;
        }
        Ok(())
    }

    fn prepare_sweep(&mut self, marks: &[bool]) -> Result<()> {
        if self.object_payloads.len() > marks.len() {
            return Err(Error::runtime("host payload mark bitmap length mismatch"));
        }
        let removed_instances = self
            .object_payloads
            .iter()
            .enumerate()
            .filter(|(index, payload)| {
                payload.is_some() && !marks.get(*index).copied().unwrap_or(false)
            })
            .count();
        self.payloads.reserve_removals(removed_instances)
    }

    fn sweep_prepared(&mut self, marks: &[bool]) -> Result<()> {
        for index in 0..self.object_payloads.len() {
            if marks.get(index).copied().unwrap_or(false) {
                continue;
            }
            let Some(slot) = self.object_payloads.get_mut(index) else {
                return Err(Error::runtime(HOST_PAYLOAD_ASSOCIATION_ERROR));
            };
            let Some(association) = slot.take() else {
                continue;
            };
            let id = association.payload;
            let Some(entry) = self.payloads.get_mut(id.index()) else {
                return Err(Error::runtime("host payload sweep entry is missing"));
            };
            entry.wrapper_count = entry
                .wrapper_count
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("host payload wrapper count underflowed"))?;
            if entry.wrapper_count == 0 {
                let removed = self.payloads.remove_reserved(id.index())?;
                if removed.is_none() {
                    return Err(Error::runtime("host payload sweep failed"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn visit_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        object_index: usize,
        visitor: &mut V,
    ) -> Result<()> {
        let Some(association) = self
            .object_payloads
            .get(object_index)
            .and_then(Option::as_ref)
        else {
            return Ok(());
        };
        if self.payloads.get(association.payload.index()).is_none() {
            return Err(Error::runtime("host payload trace entry is missing"));
        }
        for value in &association.traced_values {
            visitor.visit(
                VmObjectEdgeKind::InternalSlot,
                StrongEdgeReference::Value(value),
            )?;
        }
        Ok(())
    }

    pub(super) fn instance_count(&self) -> usize {
        self.object_payloads
            .iter()
            .filter(|payload| payload.is_some())
            .count()
    }

    pub(super) const fn payload_count(&self) -> usize {
        self.payloads.len()
    }

    pub(super) fn logical_payload_bytes(&self) -> Result<usize> {
        self.payloads.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.logical_bytes)
                .ok_or_else(|| Error::limit("host payload logical bytes overflowed"))
        })
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn create_host_object(
        &mut self,
        payload: Box<dyn Any>,
        traced_values: Vec<Value>,
        logical_bytes: usize,
        prototype: Option<Value>,
        max_objects: usize,
    ) -> Result<ObjectId> {
        self.reserve_created_object_rollback()?;
        let object_index = self.objects.next_index();
        self.host_payloads
            .attach_new(object_index, payload, traced_values, logical_bytes)?;
        let mut object = Object::ordinary();
        object.prototype = prototype;
        match self.push_object(object, max_objects) {
            Ok(id) if id.index() == object_index => Ok(id),
            Ok(id) => {
                self.discard_host_object_parts(id, object_index)?;
                Err(Error::runtime("host object allocation index changed"))
            }
            Err(error) => {
                self.host_payloads.detach_created(object_index)?;
                Err(error)
            }
        }
    }

    pub(in crate::runtime) fn clone_host_object(
        &mut self,
        source: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        self.host_payloads.payload_id(source.index())?;
        let prototype = self.object(source)?.prototype.clone();
        self.reserve_created_object_rollback()?;
        let object_index = self.objects.next_index();
        self.host_payloads
            .attach_shared(object_index, source.index())?;
        let mut object = Object::ordinary();
        object.prototype = prototype;
        match self.push_object(object, max_objects) {
            Ok(id) if id.index() == object_index => Ok(id),
            Ok(id) => {
                self.discard_host_object_parts(id, object_index)?;
                Err(Error::runtime(
                    "shared host object allocation index changed",
                ))
            }
            Err(error) => {
                self.host_payloads.detach_created(object_index)?;
                Err(error)
            }
        }
    }

    pub(crate) fn host_payload<T: 'static>(&self, id: ObjectId) -> Result<&T> {
        self.validate_id(id)?;
        self.host_payloads.payload(id.index())
    }

    pub(in crate::runtime) fn update_host_payload_bytes(
        &mut self,
        id: ObjectId,
        logical_bytes: usize,
    ) -> Result<()> {
        self.validate_id(id)?;
        self.host_payloads
            .update_logical_bytes(id.index(), logical_bytes)
    }

    pub(in crate::runtime) fn discard_host_object(&mut self, id: ObjectId) -> Result<()> {
        self.validate_id(id)?;
        self.discard_host_object_parts(id, id.index())
    }

    fn discard_host_object_parts(&mut self, id: ObjectId, association_index: usize) -> Result<()> {
        self.host_payloads.detach_created(association_index)?;
        self.discard_created_empty_object(id)
    }

    pub(in crate::runtime) fn prepare_host_payload_sweep(&mut self, marks: &[bool]) -> Result<()> {
        self.host_payloads.prepare_sweep(marks)
    }

    pub(in crate::runtime) fn sweep_host_payloads(&mut self, marks: &[bool]) -> Result<()> {
        self.host_payloads.sweep_prepared(marks)
    }

    pub(in crate::runtime) fn visit_host_payload_edges<V: StrongEdgeVisitor<VmObjectEdgeKind>>(
        &self,
        id: ObjectId,
        visitor: &mut V,
    ) -> Result<()> {
        self.host_payloads.visit_edges(id.index(), visitor)
    }
}

impl Context {
    pub(crate) fn create_typed_host_object<T: 'static>(
        &mut self,
        payload: T,
        logical_bytes: usize,
        prototype: crate::runtime::EmbeddingObjectPrototype<'_>,
        traced_values: &[crate::RetainedValue],
    ) -> Result<crate::RetainedValue> {
        let prototype = self.resolve_embedding_object_prototype(prototype)?;
        let mut edges = Vec::new();
        edges
            .try_reserve(traced_values.len())
            .map_err(|_| Error::limit("host payload traced-value capacity exceeded"))?;
        for value in traced_values {
            edges.push(self.resolve_retained_value(value)?);
        }
        let id = self.objects.create_host_object(
            Box::new(payload),
            edges,
            logical_bytes,
            prototype,
            self.limits.max_objects,
        )?;
        match self.retain_embedder_value(Value::Object(id)) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.objects.discard_host_object(id)?;
                Err(error)
            }
        }
    }

    pub(crate) fn clone_typed_host_object(
        &mut self,
        source: &crate::RetainedValue,
    ) -> Result<crate::RetainedValue> {
        let Value::Object(source) = self.resolve_retained_value(source)? else {
            return Err(Error::runtime(HOST_OBJECT_TYPE_ERROR));
        };
        let id = self
            .objects
            .clone_host_object(source, self.limits.max_objects)?;
        match self.retain_embedder_value(Value::Object(id)) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.objects.discard_host_object(id)?;
                Err(error)
            }
        }
    }

    pub(crate) fn typed_host_payload<T: 'static>(
        &self,
        object: &crate::RetainedValue,
    ) -> Result<&T> {
        let Value::Object(id) = self.resolve_retained_value(object)? else {
            return Err(Error::runtime(HOST_OBJECT_TYPE_ERROR));
        };
        self.objects.host_payload(id)
    }

    pub(crate) fn update_typed_host_payload_bytes(
        &mut self,
        object: &crate::RetainedValue,
        logical_bytes: usize,
    ) -> Result<()> {
        let Value::Object(id) = self.resolve_retained_value(object)? else {
            return Err(Error::runtime(HOST_OBJECT_TYPE_ERROR));
        };
        self.objects.update_host_payload_bytes(id, logical_bytes)
    }
}
