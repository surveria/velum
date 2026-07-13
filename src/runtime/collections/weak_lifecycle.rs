use crate::{
    error::Result,
    runtime::{Context, VmStorageKind},
    value::Value,
};

use super::CollectionId;

impl Context {
    pub(in crate::runtime) fn initialize_finalization_registry(
        &mut self,
        id: CollectionId,
        cleanup_callback: Value,
    ) -> Result<()> {
        self.collection_mut(id)?
            .initialize_finalization_registry(cleanup_callback)
    }

    pub(in crate::runtime) fn initialize_weak_ref(
        &mut self,
        id: CollectionId,
        target: Value,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionEntry, 1)?;
        if let Err(error) = self.collection_mut(id)?.initialize_weak_ref(target) {
            self.storage_ledger
                .release_count(VmStorageKind::CollectionEntry, 1)?;
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::runtime) fn weak_ref_target(&self, id: CollectionId) -> Result<Value> {
        self.collection(id)?.weak_ref_target()
    }

    pub(in crate::runtime) fn register_finalization(
        &mut self,
        id: CollectionId,
        target: Value,
        held_value: Value,
        unregister_token: Option<Value>,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionEntry, 1)?;
        if let Err(error) =
            self.collection_mut(id)?
                .register_finalization(target, held_value, unregister_token)
        {
            self.storage_ledger
                .release_count(VmStorageKind::CollectionEntry, 1)?;
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::runtime) fn unregister_finalizations(
        &mut self,
        id: CollectionId,
        token: &Value,
    ) -> Result<bool> {
        let removed = self.collection_mut(id)?.unregister_finalizations(token)?;
        self.storage_ledger
            .release_count(VmStorageKind::CollectionEntry, removed)?;
        Ok(removed != 0)
    }
}
