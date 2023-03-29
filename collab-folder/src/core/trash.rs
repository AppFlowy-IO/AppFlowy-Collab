use crate::core::{TrashInfo, ViewsMap};
use anyhow::bail;
use collab::preclude::array::ArrayEvent;
use collab::preclude::{
  lib0Any, Array, ArrayRefWrapper, Change, Observable, ReadTxn, Subscription, TransactionMut,
  Value, YrsValue,
};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::broadcast;

pub type TrashChangeSender = broadcast::Sender<TrashChange>;
pub type TrashChangeReceiver = broadcast::Receiver<TrashChange>;
type ArraySubscription = Subscription<Arc<dyn Fn(&TransactionMut, &ArrayEvent)>>;

#[derive(Debug, Clone)]
pub enum TrashChange {
  DidCreateTrash { ids: Vec<String> },
  DidDeleteTrash { ids: Vec<String> },
}

pub struct TrashArray {
  container: ArrayRefWrapper,
  view_map: Rc<ViewsMap>,
  #[allow(dead_code)]
  tx: Option<TrashChangeSender>,
  #[allow(dead_code)]
  subscription: Option<ArraySubscription>,
}

impl TrashArray {
  pub fn new(
    mut root: ArrayRefWrapper,
    view_map: Rc<ViewsMap>,
    tx: Option<TrashChangeSender>,
  ) -> Self {
    let subscription = subscribe_change(&mut root, tx.clone());
    Self {
      container: root,
      view_map,
      tx,
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

    if let Some(tx) = self.tx.as_ref() {
      let record_ids = ids
        .iter()
        .map(|id| id.as_ref().to_string())
        .collect::<Vec<String>>();
      let _ = tx.send(TrashChange::DidDeleteTrash { ids: record_ids });
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
      self.container.push_with_txn(txn, record);
    }

    if let Some(tx) = self.tx.as_ref() {
      let _ = tx.send(TrashChange::DidCreateTrash { ids: record_ids });
    }
  }

  pub fn clear(&self) {
    self.container.with_transact_mut(|txn| {
      let len = self.container.iter(txn).count();
      self.container.remove_range(txn, 0, len as u32);
    });
  }
}

fn subscribe_change(
  array: &mut ArrayRefWrapper,
  _tx: Option<TrashChangeSender>,
) -> Option<ArraySubscription> {
  Some(array.observe(move |txn, event| {
    for change in event.delta(txn) {
      match change {
        Change::Added(_) => {
          // let records = values
          //     .iter()
          //     .flat_map(|value| match value {
          //         Value::Any(any) => Some(any),
          //         _ => None,
          //     })
          //     .map(|any| TrashRecord::from(any.clone()))
          //     .map(|record| record.id)
          //     .collect::<Vec<String>>();
          // let _ = tx.send(TrashChange::DidCreateTrash { ids: records });
        },
        Change::Removed(_) => {},
        Change::Retain(_) => {},
      }
    }
  }))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashRecord {
  pub id: String,
  pub created_at: i64,
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
