use crate::core::folder_observe::{FavoriteChange, FavoriteChangeSender};
use crate::core::{subscribe_favorite_change, FavoritesInfo, ViewsMap};
use anyhow::bail;
use collab::preclude::{
  array::ArrayEvent, lib0Any, Array, ArrayRefWrapper, ReadTxn, Subscription, TransactionMut, Value,
  YrsValue,
};

use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;

type ArraySubscription = Subscription<Arc<dyn Fn(&TransactionMut, &ArrayEvent)>>;
pub struct FavoritesArray {
  container: ArrayRefWrapper,
  view_map: Rc<ViewsMap>,
  #[allow(dead_code)]
  change_tx: FavoriteChangeSender,
  #[allow(dead_code)]
  subscription: ArraySubscription,
}

impl FavoritesArray {
  pub fn new(
    mut root: ArrayRefWrapper,
    view_map: Rc<ViewsMap>,
    change_tx: FavoriteChangeSender,
  ) -> Self {
    let subscription = subscribe_favorite_change(&mut root, change_tx.clone());
    Self {
      container: root,
      view_map,
      change_tx,
      subscription,
    }
  }
  ///Gets all favorite views in form of FavoriteRecord[]
  pub fn get_all_favorites(&self) -> Vec<FavoritesInfo> {
    let txn = self.container.transact();
    let items = self.get_all_favorites_with_txn(&txn);
    items
      .into_iter()
      .map(|item| {
        let name = self
          .view_map
          .get_view_name_with_txn(&txn, &item.id)
          .unwrap_or_default();

        FavoritesInfo {
          id: item.id,
          name,
          created_at: item.created_at,
        }
      })
      .collect::<Vec<_>>()
  }
  pub fn get_all_favorites_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FavoriteRecord> {
    let mut favorites = vec![];
    for value in self.container.iter(txn) {
      if let YrsValue::Any(any) = value {
        favorites.push(FavoriteRecord::from(any))
      }
    }
    favorites
  }

  /// Deletes a favorited record to be used when a view / views is / are unfavorited
  pub fn delete_favorites<T: AsRef<str>>(&self, ids: Vec<T>) {
    self.container.with_transact_mut(|txn| {
      self.delete_favorites_with_txn(txn, ids);
    })
  }

  pub fn delete_favorites_with_txn<T: AsRef<str>>(&self, txn: &mut TransactionMut, ids: Vec<T>) {
    for id in &ids {
      if let Some(pos) = self
        .get_all_favorites_with_txn(txn)
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

    let _ = self
      .change_tx
      .send(FavoriteChange::DidUnFavoriteView { ids: record_ids });
  }

  /// Adds a favorited record to be used when a view / views is / are favorited
  pub fn add_favorites(&self, favorite_records: Vec<FavoriteRecord>) {
    self.container.with_transact_mut(|txn| {
      self.add_favorites_with_txn(txn, favorite_records);
    })
  }

  pub fn add_favorites_with_txn(
    &self,
    txn: &mut TransactionMut,
    favorite_records: Vec<FavoriteRecord>,
  ) {
    let favorite_record_ids = favorite_records
      .iter()
      .map(|favorite_record| favorite_record.id.clone())
      .collect::<Vec<String>>();

    for favorite_record in favorite_records {
      self.container.push_back(txn, favorite_record);
    }

    let _ = self.change_tx.send(FavoriteChange::DidFavoriteView {
      ids: favorite_record_ids,
    });
  }

  pub fn clear(&self) {
    self.container.with_transact_mut(|txn| {
      let len = self.container.iter(txn).count();
      self.container.remove_range(txn, 0, len as u32);
    });
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FavoriteRecord {
  pub id: String,
  pub created_at: i64,
  #[serde(default)]
  pub workspace_id: String,
}

impl From<lib0Any> for FavoriteRecord {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<FavoriteRecord> for lib0Any {
  fn from(item: FavoriteRecord) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

impl TryFrom<&YrsValue> for FavoriteRecord {
  type Error = anyhow::Error;

  fn try_from(value: &Value) -> Result<Self, Self::Error> {
    match value {
      Value::Any(any) => Ok(FavoriteRecord::from(any.clone())),
      _ => bail!("Invalid favorite yrs value"),
    }
  }
}
