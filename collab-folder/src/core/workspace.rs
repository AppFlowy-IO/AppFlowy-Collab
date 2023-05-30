use crate::core::{RepeatedView, ViewIdentifier, ViewRelations};

use collab::preclude::{MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

#[derive(Clone)]
pub struct WorkspaceMap {
  container: MapRefWrapper,
  view_relations: Rc<ViewRelations>,
}

const WORKSPACE_ID: &str = "id";
const WORKSPACE_NAME: &str = "name";
const WORKSPACE_CREATED_AT: &str = "created_at";

impl WorkspaceMap {
  pub fn new(container: MapRefWrapper, view_relations: Rc<ViewRelations>) -> Self {
    Self {
      container,
      view_relations,
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
    container: &MapRef,
    workspace_id: &str,
    view_relations: Rc<ViewRelations>,
    f: F,
  ) -> Self
  where
    F: FnOnce(WorkspaceBuilder) -> WorkspaceMap,
  {
    let builder = WorkspaceBuilder::new(workspace_id, txn, container, view_relations);
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
      let update = WorkspaceUpdate::new(
        &workspace_id,
        txn,
        &self.container,
        self.view_relations.clone(),
      );
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

    let child_views = self
      .view_relations
      .get_children_with_txn(txn, &id)
      .map(|array| array.get_children())
      .unwrap_or_default();

    Some(Workspace {
      id,
      name,
      child_views,
      created_at,
    })
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
  pub id: String,
  pub name: String,
  pub child_views: RepeatedView,
  pub created_at: i64,
}

pub struct WorkspaceBuilder<'a, 'b> {
  workspace_id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
  view_relations: Rc<ViewRelations>,
}

impl<'a, 'b> WorkspaceBuilder<'a, 'b> {
  pub fn new(
    workspace_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: &'a MapRef,
    view_relations: Rc<ViewRelations>,
  ) -> Self {
    map_ref.insert_str_with_txn(txn, WORKSPACE_ID, workspace_id);
    Self {
      workspace_id,
      map_ref,
      txn,
      view_relations,
    }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(WorkspaceUpdate),
  {
    let update = WorkspaceUpdate::new(
      self.workspace_id,
      self.txn,
      self.map_ref,
      self.view_relations.clone(),
    );
    f(update);
    self
  }
}

pub struct WorkspaceUpdate<'a, 'b, 'c> {
  workspace_id: &'a str,
  map_ref: &'c MapRef,
  txn: &'a mut TransactionMut<'b>,
  view_relations: Rc<ViewRelations>,
}

impl<'a, 'b, 'c> WorkspaceUpdate<'a, 'b, 'c> {
  pub fn new(
    workspace_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: &'c MapRef,
    view_relations: Rc<ViewRelations>,
  ) -> Self {
    Self {
      workspace_id,
      map_ref,
      txn,
      view_relations,
    }
  }

  pub fn set_name<T: AsRef<str>>(self, name: T) -> Self {
    self
      .map_ref
      .insert_str_with_txn(self.txn, WORKSPACE_NAME, name.as_ref());
    self
  }

  pub fn set_created_at(self, created_at: i64) -> Self {
    self
      .map_ref
      .insert_i64_with_txn(self.txn, WORKSPACE_CREATED_AT, created_at);
    self
  }

  pub fn set_children(self, children: RepeatedView) -> Self {
    let array = self
      .view_relations
      .get_or_create_children_with_txn(self.txn, self.workspace_id);
    array.add_children_with_txn(self.txn, children.into_inner());
    self
  }

  pub fn delete_child(self, index: u32) -> Self {
    self
      .view_relations
      .delete_children_with_txn(self.txn, self.workspace_id, index);
    self
  }

  pub fn add_children(self, belongings: Vec<ViewIdentifier>) {
    self
      .view_relations
      .add_children(self.txn, self.workspace_id, belongings);
  }
}
