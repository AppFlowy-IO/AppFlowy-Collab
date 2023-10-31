use crate::UserId;
use anyhow::bail;
use collab::preclude::{
  lib0Any, Array, Map, MapRefWrapper, ReadTxn, TransactionMut, Value, YrsValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type FavoritesByUid = HashMap<UserId, Vec<String>>;

pub struct FavoritesMap {
  uid: UserId,
  container: MapRefWrapper,
}

impl FavoritesMap {
  pub fn new(uid: &UserId, root: MapRefWrapper) -> Self {
    Self {
      uid: uid.clone(),
      container: root,
    }
  }

  pub fn get_favorite_data_with_txn<T: ReadTxn>(&self, txn: &T) -> FavoritesByUid {
    let mut favorites_by_uid = HashMap::new();
    for (uid, value) in self.container.iter(txn) {
      if let Value::YArray(array) = value {
        let mut favorites = vec![];
        for value in array.iter(txn) {
          if let YrsValue::Any(any) = value {
            favorites.push(FavoriteId::from(any))
          }
        }

        favorites_by_uid.insert(
          UserId(uid.to_string()),
          favorites.into_iter().map(|item| item.id).collect(),
        );
      }
    }
    favorites_by_uid
  }

  pub fn is_favorite(&self, view_id: &str) -> bool {
    let txn = self.container.transact();
    self.is_favorite_with_txn(&txn, view_id)
  }

  pub fn is_favorite_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> bool {
    match self.container.get_array_ref_with_txn(txn, &self.uid) {
      None => false,
      Some(fav_array) => {
        for value in fav_array.iter(txn) {
          if let Ok(favorite_id) = FavoriteId::try_from(&value) {
            if favorite_id.id == view_id {
              return true;
            }
          }
        }
        false
      },
    }
  }

  ///Gets all favorite views in form of FavoriteRecord[]
  pub fn get_all_favorites(&self) -> Vec<FavoriteId> {
    let txn = self.container.transact();
    self.get_all_favorites_with_txn(&txn)
  }

  pub fn get_all_favorites_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FavoriteId> {
    match self.container.get_array_ref_with_txn(txn, &self.uid) {
      None => vec![],
      Some(fav_array) => {
        let mut favorites = vec![];
        for value in fav_array.iter(txn) {
          if let YrsValue::Any(any) = value {
            favorites.push(FavoriteId::from(any))
          }
        }
        favorites
      },
    }
  }

  /// Deletes a favorited record to be used when a view / views is / are unfavorited
  pub fn delete_favorites<T: AsRef<str>>(&self, ids: Vec<T>) {
    self.container.with_transact_mut(|txn| {
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
  pub fn add_favorites(&self, favorite_records: Vec<FavoriteId>) {
    self.container.with_transact_mut(|txn| {
      self.add_favorites_with_txn(txn, favorite_records);
    });
  }

  pub fn add_favorites_with_txn(
    &self,
    txn: &mut TransactionMut,
    favorite_records: Vec<FavoriteId>,
  ) {
    self.add_favorites_for_user_with_txn(txn, &self.uid, favorite_records);
  }

  pub(crate) fn add_favorites_for_user_with_txn(
    &self,
    txn: &mut TransactionMut,
    uid: &UserId,
    favorite_records: Vec<FavoriteId>,
  ) {
    let fav_array = self
      .container
      .create_array_if_not_exist_with_txn::<FavoriteId, _>(txn, uid, vec![]);

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
pub struct FavoriteId {
  pub id: String,
}

impl From<lib0Any> for FavoriteId {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<FavoriteId> for lib0Any {
  fn from(item: FavoriteId) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

impl From<&FavoriteId> for FavoriteId {
  fn from(value: &FavoriteId) -> Self {
    FavoriteId {
      id: value.id.clone(),
    }
  }
}
impl TryFrom<&YrsValue> for FavoriteId {
  type Error = anyhow::Error;

  fn try_from(value: &Value) -> Result<Self, Self::Error> {
    match value {
      Value::Any(any) => Ok(FavoriteId::from(any.clone())),
      _ => bail!("Invalid favorite yrs value"),
    }
  }
}
