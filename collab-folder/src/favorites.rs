use anyhow::bail;
use collab::preclude::{lib0Any, Array, MapRefWrapper, ReadTxn, TransactionMut, Value, YrsValue};

use serde::{Deserialize, Serialize};
use std::rc::Rc;

use crate::{UserId, ViewsMap};
pub struct FavoritesMap {
  uid: UserId,
  container: MapRefWrapper,
  view_map: Rc<ViewsMap>,
}

impl FavoritesMap {
  pub fn new(uid: &UserId, root: MapRefWrapper, view_map: Rc<ViewsMap>) -> Self {
    Self {
      uid: uid.clone(),
      container: root,
      view_map,
    }
  }
  ///Gets all favorite views in form of FavoriteRecord[]
  pub fn get_all_favorites(&self) -> Vec<FavoriteRecord> {
    let txn = self.container.transact();
    self.get_all_favorites_with_txn(&txn)
  }

  pub fn get_all_favorites_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FavoriteRecord> {
    match self.container.get_array_ref_with_txn(txn, &self.uid) {
      None => vec![],
      Some(fav_array) => {
        let mut favorites = vec![];
        for value in fav_array.iter(txn) {
          if let YrsValue::Any(any) = value {
            favorites.push(FavoriteRecord::from(any))
          }
        }
        favorites
      },
    }
  }

  /// Deletes a favorited record to be used when a view / views is / are unfavorited
  pub fn delete_favorites<T: AsRef<str>>(&self, ids: Vec<T>) {
    self.container.with_transact_mut(|txn| {
      ids.iter().for_each(|record| {
        self
          .view_map
          .update_view_with_txn(&self.uid, txn, record.as_ref(), |update| {
            update.set_favorite(false).done()
          });
      });
      self.delete_favorites_with_txn(txn, ids);
    });
  }

  pub fn delete_favorites_with_txn<T: AsRef<str>>(&self, txn: &mut TransactionMut, ids: Vec<T>) {
    if let Some(fav_array) = self.container.get_array_ref_with_txn(txn, &self.uid) {
      for id in &ids {
        if let Some(pos) = self
          .get_all_favorites_with_txn(txn)
          .into_iter()
          .position(|item| item.id == id.as_ref())
        {
          fav_array.remove_with_txn(txn, pos as u32);
        }
      }
    }
  }

  /// Adds a favorited record to be used when a view / views is / are favorited
  pub fn add_favorites(&self, uid: &UserId, favorite_records: Vec<FavoriteRecord>) {
    self.container.with_transact_mut(|txn| {
      favorite_records.iter().for_each(|record| {
        self
          .view_map
          .update_view_with_txn(uid, txn, &record.id, |update| {
            update.set_favorite(true).done()
          });
      });
      self.add_favorites_with_txn(txn, favorite_records);
    });
  }

  pub fn add_favorites_with_txn(
    &self,
    txn: &mut TransactionMut,
    favorite_records: Vec<FavoriteRecord>,
  ) {
    let fav_array = self
      .container
      .create_array_if_not_exist_with_txn::<FavoriteRecord, _>(txn, &self.uid, vec![]);

    for favorite_record in favorite_records {
      fav_array.push_back(txn, favorite_record);
    }
  }

  pub fn clear(&self) {
    self.container.with_transact_mut(|txn| {
      if let Some(fav_array) = self.container.get_array_ref_with_txn(txn, &self.uid) {
        let len = fav_array.iter(txn).count();
        fav_array.remove_range(txn, 0, len as u32);
      }
    });
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FavoriteRecord {
  pub id: String,
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

impl From<&FavoriteRecord> for FavoriteRecord {
  fn from(value: &FavoriteRecord) -> Self {
    FavoriteRecord {
      id: value.id.clone(),
      workspace_id: value.workspace_id.clone(),
    }
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
