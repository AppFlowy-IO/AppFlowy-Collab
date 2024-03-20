use collab::core::array_wrapper::ArrayRefExtension;
use collab::core::value::YrsValueExtension;
use collab::preclude::{
  Any, Array, ArrayRefWrapper, Collab, MapPrelim, MapRef, MapRefExtension, ReadTxn, TransactionMut,
  YrsValue,
};
use std::collections::HashSet;

use crate::database::timestamp;
use crate::views::CreateDatabaseParams;

const DATABASES: &str = "databases";

/// Used to store list of [DatabaseMeta].
pub struct DatabaseMetaList {
  array_ref: ArrayRefWrapper,
}

impl DatabaseMetaList {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn from_collab(collab: &Collab) -> Self {
    let array = {
      let txn = collab.transact();
      collab.get_array_with_txn(&txn, vec![DATABASES])
    };

    let databases = array.unwrap_or_else(|| {
      collab.with_origin_transact_mut(|txn| {
        collab.create_array_with_txn::<MapPrelim<Any>>(txn, DATABASES, vec![])
      })
    });

    Self::new(databases)
  }

  /// Create a new [DatabaseMeta] for the given database id and view id
  /// use [Self::update_database] to attach more views to the existing database.
  ///
  pub fn add_database(&self, params: &CreateDatabaseParams) {
    self.array_ref.with_transact_mut(|txn| {
      // Use HashSet to remove duplicates
      let mut linked_views = HashSet::new();
      linked_views.insert(params.inline_view_id.to_string());
      linked_views.extend(
        params
          .views
          .iter()
          .filter(|view| view.view_id != params.inline_view_id)
          .map(|view| view.view_id.clone()),
      );
      let record = DatabaseMeta {
        database_id: params.database_id.clone(),
        created_at: timestamp(),
        linked_views: linked_views.into_iter().collect(),
      };
      let map_ref = self.array_ref.insert_map_with_txn(txn, None);
      record.fill_map_ref(txn, &map_ref);
    });
  }

  /// Update the database by the given id
  pub fn update_database(&self, database_id: &str, mut f: impl FnMut(&mut DatabaseMeta)) {
    self.array_ref.with_transact_mut(|txn| {
      if let Some(index) = self.database_index_from_id(txn, database_id) {
        if let Some(Some(map_ref)) = self
          .array_ref
          .get(txn, index)
          .map(|value| value.to_ymap().cloned())
        {
          if let Some(mut record) = DatabaseMeta::from_map_ref(txn, &map_ref) {
            f(&mut record);
            self.array_ref.remove(txn, index);
            let map_ref = self
              .array_ref
              .insert_map_at_index_with_txn(txn, index, None);
            record.fill_map_ref(txn, &map_ref);
          }
        }
      }
    });
  }

  /// Delete the database by the given id
  pub fn delete_database(&self, database_id: &str) {
    self.array_ref.with_transact_mut(|txn| {
      if let Some(index) = self.database_index_from_id(txn, database_id) {
        self.array_ref.remove(txn, index);
      }
    });
  }

  /// Return all the database meta
  pub fn get_all_database_meta(&self) -> Vec<DatabaseMeta> {
    self
      .array_ref
      .with_transact_mut(|txn| self.get_all_database_meta_with_txn(txn))
  }

  /// Test if the database with the given id exists
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

  /// Return all databases with a Transaction
  pub fn get_all_database_meta_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseMeta> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| {
        let map_ref = value.to_ymap()?;
        DatabaseMeta::from_map_ref(txn, map_ref)
      })
      .collect()
  }

  /// Return the a [DatabaseMeta] with the given view id
  pub fn get_database_meta_with_view_id(&self, view_id: &str) -> Option<DatabaseMeta> {
    let txn = self.array_ref.transact();
    let all = self.get_all_database_meta_with_txn(&txn);
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
    map_ref.insert_str_with_txn(txn, DATABASE_TRACKER_ID, self.database_id);
    map_ref.insert_str_with_txn(txn, DATABASE_RECORD_CREATED_AT, self.created_at);
    let views = self.linked_views.into_iter().collect::<Vec<String>>();
    map_ref.create_array_with_txn(txn, DATABASE_RECORD_VIEWS, views);
  }

  fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<Self> {
    let database_id = map_ref.get_str_with_txn(txn, DATABASE_TRACKER_ID)?;
    let created_at = map_ref
      .get_i64_with_txn(txn, DATABASE_RECORD_CREATED_AT)
      .unwrap_or_default();
    let linked_views = map_ref
      .get_array_ref_with_txn(txn, DATABASE_RECORD_VIEWS)?
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
    map_ref.get_str_with_txn(txn, DATABASE_TRACKER_ID)
  } else {
    None
  }
}
