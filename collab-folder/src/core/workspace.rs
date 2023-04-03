use crate::core::{Belonging, BelongingMap, Belongings, View};
use anyhow::Result;
use collab::preclude::{MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Clone)]
pub struct WorkspaceMap {
  container: MapRefWrapper,
  belongings: Rc<BelongingMap>,
}

const WORKSPACE_ID: &str = "id";
const WORKSPACE_NAME: &str = "name";
const WORKSPACE_CREATED_AT: &str = "created_at";

impl WorkspaceMap {
  pub fn new(container: MapRefWrapper, belongings: Rc<BelongingMap>) -> Self {
    Self {
      container,
      belongings,
    }
  }

  pub fn workspace_id(&self) -> Option<String> {
    let txn = self.container.transact();
    self.container.get_str_with_txn(&txn, WORKSPACE_ID)
  }

  pub fn get_workspace_id_with_txn<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    self.container.get_str_with_txn(txn, WORKSPACE_ID)
  }

  pub fn create_with_txn<F>(
    txn: &mut TransactionMut,
    container: MapRefWrapper,
    workspace_id: &str,
    belongings: Rc<BelongingMap>,
    f: F,
  ) -> Self
  where
    F: FnOnce(WorkspaceBuilder) -> WorkspaceMap,
  {
    let builder = WorkspaceBuilder::new(workspace_id, txn, container, belongings);
    f(builder)
  }

  pub fn update<F>(&self, f: F)
  where
    F: FnOnce(WorkspaceUpdate),
  {
    self
      .container
      .with_transact_mut(|txn| self.update_with_txn(txn, f))
  }

  pub fn update_with_txn<F>(&self, txn: &mut TransactionMut, f: F)
  where
    F: FnOnce(WorkspaceUpdate),
  {
    if let Some(workspace_id) = self.get_workspace_id_with_txn(txn) {
      let update =
        WorkspaceUpdate::new(&workspace_id, txn, &self.container, self.belongings.clone());
      f(update);
    }
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

    let belongings = self
      .belongings
      .get_belongings_array_with_txn(txn, &id)
      .map(|array| array.get_belongings())
      .unwrap_or_default();

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

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
  pub id: String,
  pub name: String,
  pub views: Vec<View>,
  pub created_at: i64,
}

pub struct WorkspaceBuilder<'a, 'b> {
  workspace_id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  belongings: Rc<BelongingMap>,
}

impl<'a, 'b> WorkspaceBuilder<'a, 'b> {
  pub fn new(
    workspace_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRefWrapper,
    belongings: Rc<BelongingMap>,
  ) -> Self {
    map_ref.insert_with_txn(txn, WORKSPACE_ID, workspace_id);
    Self {
      workspace_id,
      map_ref,
      txn,
      belongings,
    }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(WorkspaceUpdate),
  {
    let update = WorkspaceUpdate::new(
      self.workspace_id,
      self.txn,
      &self.map_ref,
      self.belongings.clone(),
    );
    f(update);
    self
  }

  pub fn done(self) -> Result<WorkspaceMap> {
    Ok(WorkspaceMap::new(self.map_ref, self.belongings))
  }
}

pub struct WorkspaceUpdate<'a, 'b, 'c> {
  workspace_id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  belongings: Rc<BelongingMap>,
}

impl<'a, 'b, 'c> WorkspaceUpdate<'a, 'b, 'c> {
  pub fn new(
    workspace_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: &'c MapRefWrapper,
    belongings: Rc<BelongingMap>,
  ) -> Self {
    Self {
      workspace_id,
      map_ref,
      txn,
      belongings,
    }
  }

  pub fn set_name<T: AsRef<str>>(self, name: T) -> Self {
    self
      .map_ref
      .insert_with_txn(self.txn, WORKSPACE_NAME, name.as_ref());
    self
  }

  pub fn set_created_at(self, created_at: i64) -> Self {
    self
      .map_ref
      .insert_with_txn(self.txn, WORKSPACE_CREATED_AT, created_at);
    self
  }

  pub fn set_belongings(self, belongings: Belongings) -> Self {
    let array = self
      .belongings
      .get_or_create_belongings_with_txn(self.txn, self.workspace_id);
    array.add_belongings_with_txn(self.txn, belongings.into_inner());
    self
  }

  pub fn delete_belongings(self, index: u32) -> Self {
    self
      .belongings
      .delete_belongings_with_txn(self.txn, self.workspace_id, index);
    self
  }

  pub fn add_belongings(self, belongings: Vec<Belonging>) {
    self
      .belongings
      .add_belongings(self.txn, self.workspace_id, belongings);
  }
}
