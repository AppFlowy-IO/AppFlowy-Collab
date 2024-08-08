use std::collections::HashSet;

use collab::preclude::{
  Array, ArrayPrelim, ArrayRef, Collab, Map, MapExt, MapPrelim, MapRef, ReadTxn, TransactionMut,
  YrsValue,
};
use collab_entity::define::WORKSPACE_DATABASES;

use crate::database::timestamp;

/// Used to store list of [DatabaseMeta].
pub struct DatabaseMetaList {
  array_ref: ArrayRef,
}

impl DatabaseMetaList {
  pub fn new(collab: &mut Collab) -> Self {
    let mut txn = collab.context.transact_mut();
    let array_ref = collab.data.get_or_init(&mut txn, WORKSPACE_DATABASES);
    Self { array_ref }
  }

  /// Create a new [DatabaseMeta] for the given database id and view id
  /// use [Self::update_database] to attach more views to the existing database.
  ///
  pub fn add_database(&self, txn: &mut TransactionMut, database_id: &str, view_ids: Vec<String>) {
    // Use HashSet to remove duplicates
    let linked_views: HashSet<String> = view_ids.into_iter().collect();
    let record = DatabaseMeta {
      database_id: database_id.to_string(),
      created_at: timestamp(),
      linked_views: linked_views.into_iter().collect(),
    };
    let map_ref: MapRef = self.array_ref.push_back(txn, MapPrelim::default());
    record.fill_map_ref(txn, &map_ref);
  }

  /// Update the database by the given id
  pub fn update_database(
    &self,
    txn: &mut TransactionMut,
    database_id: &str,
    mut f: impl FnMut(&mut DatabaseMeta),
  ) {
    if let Some(index) = self.database_index_from_id(txn, database_id) {
      if let Some(Some(map_ref)) = self
        .array_ref
        .get(txn, index)
        .map(|value| value.cast().ok())
      {
        if let Some(mut record) = DatabaseMeta::from_map_ref(txn, &map_ref) {
          f(&mut record);
          self.array_ref.remove(txn, index);
          let map_ref = self.array_ref.insert(txn, index, MapPrelim::default());
          record.fill_map_ref(txn, &map_ref);
        }
      }
    }
  }

  /// Delete the database by the given id
  pub fn delete_database(&self, txn: &mut TransactionMut, database_id: &str) {
    if let Some(index) = self.database_index_from_id(txn, database_id) {
      self.array_ref.remove(txn, index);
    }
  }

  /// Test if the database with the given id exists
  pub fn contains<T: ReadTxn>(&self, txn: &T, database_id: &str) -> bool {
    self
      .array_ref
      .iter(txn)
      .any(|value| match database_id_from_value(txn, value) {
        None => false,
        Some(id) => id == database_id,
      })
  }

  /// Return all databases with a Transaction
  pub fn get_all_database_meta<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseMeta> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| {
        let map_ref: MapRef = value.cast().ok()?;
        DatabaseMeta::from_map_ref(txn, &map_ref)
      })
      .collect()
  }

  /// Return the a [DatabaseMeta] with the given view id
  pub fn get_database_meta_with_view_id<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
  ) -> Option<DatabaseMeta> {
    let all = self.get_all_database_meta(txn);
    all
      .into_iter()
      .find(|record| record.linked_views.iter().any(|id| id == view_id))
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

/// [DatabaseMeta] is a structure used to manage and track the metadata of views associated with a particular database.
/// It's primarily used to maintain a record of all views that are attached to a database, facilitating easier tracking and management.
///
#[derive(Clone, Debug)]
pub struct DatabaseMeta {
  pub database_id: String,
  pub created_at: i64,
  pub linked_views: Vec<String>,
}

const DATABASE_TRACKER_ID: &str = "database_id";
const DATABASE_RECORD_CREATED_AT: &str = "created_at";
const DATABASE_RECORD_VIEWS: &str = "views";

impl DatabaseMeta {
  fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    map_ref.insert(txn, DATABASE_TRACKER_ID, self.database_id);
    map_ref.insert(txn, DATABASE_RECORD_CREATED_AT, self.created_at);
    map_ref.insert(
      txn,
      DATABASE_RECORD_VIEWS,
      ArrayPrelim::from_iter(self.linked_views),
    );
  }

  fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<Self> {
    let database_id: String = map_ref.get_with_txn(txn, DATABASE_TRACKER_ID)?;
    let created_at: i64 = map_ref
      .get_with_txn(txn, DATABASE_RECORD_CREATED_AT)
      .unwrap_or_default();
    let linked_views = map_ref
      .get_with_txn::<_, ArrayRef>(txn, DATABASE_RECORD_VIEWS)?
      .iter(txn)
      .map(|value| value.to_string(txn))
      .collect();

    Some(Self {
      database_id,
      created_at,
      linked_views,
    })
  }
}

fn database_id_from_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<String> {
  if let YrsValue::YMap(map_ref) = value {
    map_ref.get_with_txn(txn, DATABASE_TRACKER_ID)
  } else {
    None
  }
}
