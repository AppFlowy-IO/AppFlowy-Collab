use crate::database::timestamp;

use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  Array, ArrayRefWrapper, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue,
};
use std::collections::HashSet;

pub struct DatabaseArray {
  array_ref: ArrayRefWrapper,
}

impl DatabaseArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn add_database(&self, database_id: &str, view_id: &str, name: &str) {
    self.array_ref.with_transact_mut(|txn| {
      let mut views = HashSet::new();
      views.insert(view_id.to_string());
      let record = DatabaseRecord {
        database_id: database_id.to_string(),
        name: name.to_string(),
        created_at: timestamp(),
        views,
      };
      let map_ref = self.array_ref.insert_map_with_txn(txn);
      record.fill_map_ref(txn, &map_ref);
    });
  }

  pub fn update_database(&self, database_id: &str, mut f: impl FnMut(&mut DatabaseRecord)) {
    self.array_ref.with_transact_mut(|txn| {
      if let Some(index) = self.database_index_from_id(txn, database_id) {
        if let Some(Some(map_ref)) = self.array_ref.get(txn, index).map(|value| value.to_ymap()) {
          if let Some(mut record) = DatabaseRecord::from_map_ref(txn, &map_ref) {
            f(&mut record);
            self.array_ref.remove(txn, index as u32);
            let map_ref = self.array_ref.insert_map_at_index_with_txn(txn, index);
            record.fill_map_ref(txn, &map_ref);
          }
        }
      }
    });
  }

  pub fn delete_database(&self, database_id: &str) {
    self.array_ref.with_transact_mut(|txn| {
      if let Some(index) = self.database_index_from_id(txn, database_id) {
        self.array_ref.remove(txn, index);
      }
    });
  }

  pub fn get_all_databases(&self) -> Vec<DatabaseRecord> {
    self
      .array_ref
      .with_transact_mut(|txn| self.get_all_databases_with_txn(txn))
  }

  pub fn contains(&self, database_id: &str) -> bool {
    let txn = self.array_ref.transact();
    self
      .array_ref
      .iter(&txn)
      .any(|value| match database_id_from_value(&txn, value) {
        None => false,
        Some(id) => id == database_id,
      })
  }

  pub fn get_all_databases_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseRecord> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| {
        let map_ref = value.to_ymap()?;
        DatabaseRecord::from_map_ref(txn, &map_ref)
      })
      .collect()
  }

  pub fn get_database_record_with_view_id(&self, view_id: &str) -> Option<DatabaseRecord> {
    let txn = self.array_ref.transact();
    let all = self.get_all_databases_with_txn(&txn);
    all
      .into_iter()
      .find(|record| record.views.contains(view_id))
  }

  fn database_index_from_id<T: ReadTxn>(&self, txn: &T, database_id: &str) -> Option<u32> {
    self
      .array_ref
      .iter(txn)
      .position(|value| match database_id_from_value(txn, value) {
        None => false,
        Some(id) => id == database_id,
      })
      .map(|index| index as u32)
  }
}

pub struct DatabaseRecord {
  pub database_id: String,
  pub name: String,
  pub created_at: i64,
  pub views: HashSet<String>,
}

const DATABASE_RECORD_ID: &str = "database_id";
const DATABASE_RECORD_NAME: &str = "name";
const DATABASE_RECORD_CREATED_AT: &str = "created_at";
const DATABASE_RECORD_VIEWS: &str = "views";

impl DatabaseRecord {
  fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    map_ref.insert_str_with_txn(txn, DATABASE_RECORD_ID, self.database_id);
    map_ref.insert_str_with_txn(txn, DATABASE_RECORD_NAME, self.name);
    map_ref.insert_str_with_txn(txn, DATABASE_RECORD_CREATED_AT, self.created_at);
    let views = self.views.into_iter().collect::<Vec<String>>();
    map_ref.insert_array_with_txn(txn, DATABASE_RECORD_VIEWS, views);
  }

  #[allow(clippy::needless_collect)]
  fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<Self> {
    let id = map_ref.get_str_with_txn(txn, DATABASE_RECORD_ID)?;
    let name = map_ref
      .get_str_with_txn(txn, DATABASE_RECORD_NAME)
      .unwrap_or_default();
    let created_at = map_ref
      .get_i64_with_txn(txn, DATABASE_RECORD_CREATED_AT)
      .unwrap_or_default();
    let views = map_ref
      .get_array_ref_with_txn(txn, DATABASE_RECORD_VIEWS)?
      .iter(txn)
      .map(|value| value.to_string(txn))
      .collect::<Vec<String>>();

    Some(Self {
      database_id: id,
      name,
      created_at,
      views: views.into_iter().collect(),
    })
  }
}

fn database_id_from_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<String> {
  if let YrsValue::YMap(map_ref) = value {
    map_ref.get_str_with_txn(txn, DATABASE_RECORD_ID)
  } else {
    None
  }
}
