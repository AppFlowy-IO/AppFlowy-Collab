use crate::core::{Belongings, BelongingsArray};
use anyhow::{bail, Result};
use collab::preclude::{Array, Map, MapRef, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct WorkspaceMap {
    container: MapRefWrapper,
}

const WORKSPACE_ID: &str = "id";
const WORKSPACE_NAME: &str = "name";
const WORKSPACE_BELONGINGS: &str = "belongings";
const WORKSPACE_CREATED_AT: &str = "created_at";

impl WorkspaceMap {
    pub fn new(container: MapRefWrapper) -> Self {
        Self { container }
    }

    pub fn workspace_id(&self) -> Option<String> {
        self.container.get_str(WORKSPACE_ID)
    }

    pub fn create_with_txn<F>(
        txn: &mut TransactionMut,
        container: MapRefWrapper,
        workspace_id: &str,
        f: F,
    ) -> Self
    where
        F: FnOnce(WorkspaceBuilder) -> WorkspaceMap,
    {
        let builder = WorkspaceBuilder::new(workspace_id, txn, container.clone());
        f(builder)
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(WorkspaceUpdate),
    {
        self.container
            .with_transact_mut(|txn| self.update_with_txn(txn, f))
    }

    pub fn update_with_txn<F>(&self, txn: &mut TransactionMut, f: F)
    where
        F: FnOnce(WorkspaceUpdate),
    {
        let update = WorkspaceUpdate::new(txn, &self.container);
        f(update);
    }

    pub fn to_workspace(&self) -> Option<Workspace> {
        let txn = self.container.transact();
        self.to_workspace_with_txn(&txn)
    }

    pub fn to_workspace_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<Workspace> {
        let id = self.container.get_str_with_txn(txn, WORKSPACE_ID)?;
        let name = self
            .container
            .get_str_with_txn(txn, WORKSPACE_NAME)
            .unwrap_or_default();
        let created_at = self
            .container
            .get_i64_with_txn(txn, WORKSPACE_CREATED_AT)
            .unwrap_or_default();
        let array = self.container.get_array_ref(WORKSPACE_BELONGINGS)?;
        let belongings = BelongingsArray::from_array(array).get_belongings();
        Some(Workspace {
            id,
            name,
            belongings,
            created_at,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub belongings: Belongings,
    pub created_at: i64,
}

pub struct WorkspaceBuilder<'a, 'b> {
    map_ref: MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> WorkspaceBuilder<'a, 'b> {
    pub fn new(
        workspace_id: &str,
        txn: &'a mut TransactionMut<'b>,
        map_ref: MapRefWrapper,
    ) -> Self {
        map_ref.insert_with_txn(txn, WORKSPACE_ID, workspace_id);
        Self { map_ref, txn }
    }

    pub fn update<F>(self, f: F) -> Self
    where
        F: FnOnce(WorkspaceUpdate),
    {
        let update = WorkspaceUpdate::new(self.txn, &self.map_ref);
        f(update);
        self
    }

    pub fn done(self) -> Result<WorkspaceMap> {
        Ok(WorkspaceMap::new(self.map_ref))
    }
}

pub struct WorkspaceUpdate<'a, 'b, 'c> {
    map_ref: &'c MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
    belongings: BelongingsArray,
}

impl<'a, 'b, 'c> WorkspaceUpdate<'a, 'b, 'c> {
    pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
        let belongings = BelongingsArray::get_or_create_with_txn(txn, &map_ref);
        Self {
            map_ref,
            txn,
            belongings,
        }
    }

    pub fn set_name<T: AsRef<str>>(self, name: T) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, WORKSPACE_NAME, name.as_ref());
        self
    }

    pub fn set_created_at(self, created_at: i64) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, WORKSPACE_CREATED_AT, created_at);
        self
    }

    pub fn set_belongings(mut self, belongings: Belongings) -> Self {
        let belongings_map = self.map_ref.insert_array_with_txn(
            self.txn,
            WORKSPACE_BELONGINGS,
            belongings.into_inner(),
        );
        self.belongings = BelongingsArray::from_array(belongings_map);
        self
    }

    pub fn delete_belongings(self, index: u32) -> Self {
        self.belongings.remove_belonging_with_txn(self.txn, index);
        self
    }
}
