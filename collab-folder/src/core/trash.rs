use std::rc::Rc;
use std::sync::Arc;

use anyhow::bail;
use collab::preclude::array::ArrayEvent;
use collab::preclude::{
  lib0Any, Array, ArrayRefWrapper, ReadTxn, Subscription, TransactionMut, Value, YrsValue,
};
use serde::{Deserialize, Serialize};

use crate::core::folder_observe::{TrashChange, TrashChangeSender};
use crate::core::{subscribe_trash_change, TrashInfo, ViewsMap};

type ArraySubscription = Subscription<Arc<dyn Fn(&TransactionMut, &ArrayEvent)>>;

pub struct TrashArray {
  container: ArrayRefWrapper,
  view_map: Rc<ViewsMap>,
  #[allow(dead_code)]
  change_tx: Option<TrashChangeSender>,
  #[allow(dead_code)]
  subscription: ArraySubscription,
}

impl TrashArray {
  pub fn new(
    mut root: ArrayRefWrapper,
    view_map: Rc<ViewsMap>,
    change_tx: Option<TrashChangeSender>,
  ) -> Self {
    let subscription = subscribe_trash_change(&mut root);
    Self {
      container: root,
      view_map,
      change_tx,
      subscription,
    }
  }

  pub fn get_all_trash(&self) -> Vec<TrashInfo> {
    let txn = self.container.transact();
    let items = self.get_all_trash_with_txn(&txn);
    items
      .into_iter()
      .map(|item| {
        let name = self
          .view_map
          .get_view_name_with_txn(&txn, &item.id)
          .unwrap_or_default();
        TrashInfo {
          id: item.id,
          name,
          created_at: item.created_at,
        }
      })
      .collect::<Vec<_>>()
  }

  pub fn get_all_trash_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<TrashRecord> {
    let mut trash = vec![];
    for value in self.container.iter(txn) {
      if let YrsValue::Any(any) = value {
        trash.push(TrashRecord::from(any));
      }
    }
    trash
  }

  pub fn delete_trash<T: AsRef<str>>(&self, ids: Vec<T>) {
    self.container.with_transact_mut(|txn| {
      self.delete_trash_with_txn(txn, ids);
    })
  }

  pub fn delete_trash_with_txn<T: AsRef<str>>(&self, txn: &mut TransactionMut, ids: Vec<T>) {
    for id in &ids {
      if let Some(pos) = self
        .get_all_trash_with_txn(txn)
        .into_iter()
        .position(|item| item.id == id.as_ref())
      {
        self.container.remove_with_txn(txn, pos as u32);
      }
    }

    let record_ids = ids
      .iter()
      .map(|id| id.as_ref().to_string())
      .collect::<Vec<String>>();

    if let Some(change_tx) = &self.change_tx {
      let _ = change_tx.send(TrashChange::DidDeleteTrash { ids: record_ids });
    }
  }

  pub fn add_trash(&self, records: Vec<TrashRecord>) {
    self.container.with_transact_mut(|txn| {
      self.add_trash_with_txn(txn, records);
    })
  }

  pub fn add_trash_with_txn(&self, txn: &mut TransactionMut, records: Vec<TrashRecord>) {
    let record_ids = records
      .iter()
      .map(|record| record.id.clone())
      .collect::<Vec<String>>();
    for record in records {
      self.container.push_back(txn, record);
    }

    if let Some(change_tx) = &self.change_tx {
      let _ = change_tx.send(TrashChange::DidCreateTrash { ids: record_ids });
    }
  }

  pub fn clear(&self) {
    let ids = self
      .get_all_trash()
      .iter()
      .map(|info| info.id.to_string())
      .collect();
    self.container.with_transact_mut(|txn| {
      let len = self.container.iter(txn).count();
      self.container.remove_range(txn, 0, len as u32);
    });
    self.change_tx.send(TrashChange::DidDeleteTrash { ids });
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashRecord {
  pub id: String,
  pub created_at: i64,
  #[serde(default)]
  pub workspace_id: String,
}

impl From<lib0Any> for TrashRecord {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<TrashRecord> for lib0Any {
  fn from(item: TrashRecord) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

impl TryFrom<&YrsValue> for TrashRecord {
  type Error = anyhow::Error;

  fn try_from(value: &Value) -> Result<Self, Self::Error> {
    match value {
      Value::Any(any) => Ok(TrashRecord::from(any.clone())),
      _ => bail!("Invalid trash yrs value"),
    }
  }
}
