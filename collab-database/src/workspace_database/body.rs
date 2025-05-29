use crate::database::timestamp;
use crate::error::DatabaseError;
use anyhow::anyhow;
use collab::core::collab::{CollabOptions, DataSource};
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::{
  Array, ArrayPrelim, ArrayRef, Collab, Map, MapExt, MapPrelim, MapRef, ReadTxn, TransactionMut,
  YrsValue,
};
use collab_entity::CollabType;
use collab_entity::define::WORKSPACE_DATABASES;
use std::borrow::{Borrow, BorrowMut};
use std::collections::{HashMap, HashSet};
use yrs::block::ClientID;

/// Used to store list of [DatabaseMeta].
pub struct WorkspaceDatabase {
  pub collab: Collab,
  pub body: WorkspaceDatabaseBody,
}

pub fn default_workspace_database_data(object_id: &str, client_id: ClientID) -> EncodedCollab {
  let options = CollabOptions::new(object_id.to_string(), client_id);
  let mut collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let _ = WorkspaceDatabaseBody::create(&mut collab);
  collab
    .encode_collab_v1(|_collab| Ok::<_, DatabaseError>(()))
    .unwrap()
}

impl WorkspaceDatabase {
  pub fn open(mut collab: Collab) -> Result<Self, DatabaseError> {
    CollabType::WorkspaceDatabase.validate_require_data(&collab)?;
    let body = WorkspaceDatabaseBody::open(&mut collab)?;
    Ok(Self { body, collab })
  }

  pub fn create(mut collab: Collab) -> Self {
    let body = WorkspaceDatabaseBody::create(&mut collab);
    Self { body, collab }
  }

  pub fn from_collab_doc_state(
    object_id: &str,
    origin: CollabOrigin,
    collab_doc_state: DataSource,
    client_id: ClientID,
  ) -> Result<Self, DatabaseError> {
    let options =
      CollabOptions::new(object_id.to_string(), client_id).with_data_source(collab_doc_state);
    let collab = Collab::new_with_options(origin, options)
      .map_err(|err| DatabaseError::Internal(anyhow!("Failed to create collab: {}", err)))?;
    Self::open(collab)
  }

  pub fn close(&self) {
    self.collab.remove_all_plugins();
  }

  /// Create a new [DatabaseMeta] for the given database id and view id
  /// use [Self::update_database] to attach more views to the existing database.
  ///
  pub fn add_database(&mut self, database_id: &str, view_ids: Vec<String>) -> TransactionMut {
    let mut txn = self.collab.transact_mut();
    self.body.add_database(&mut txn, database_id, view_ids);
    txn
  }

  pub fn batch_add_database(
    &mut self,
    view_ids_by_database_id: HashMap<String, Vec<String>>,
  ) -> TransactionMut {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .batch_add_database(&mut txn, view_ids_by_database_id);
    txn
  }

  /// Update the database by the given id
  pub fn update_database(
    &mut self,
    database_id: &str,
    f: impl FnMut(&mut DatabaseMeta),
  ) -> TransactionMut {
    let mut txn = self.collab.transact_mut();
    self.body.update_database(&mut txn, database_id, f);
    txn
  }

  /// Delete the database by the given id
  pub fn delete_database(&mut self, database_id: &str) -> TransactionMut {
    let mut txn = self.collab.transact_mut();
    self.body.delete_database(&mut txn, database_id);
    txn
  }

  /// Test if the database with the given id exists
  pub fn contains(&self, database_id: &str) -> bool {
    let txn = self.collab.transact();
    self.body.contains_database(&txn, database_id)
  }

  /// Return all databases with a Transaction
  pub fn get_all_database_meta(&self) -> Vec<DatabaseMeta> {
    let txn = self.collab.transact();
    self.body.get_all_meta(&txn)
  }

  /// Return the a [DatabaseMeta] with the given view id
  pub fn get_database_meta_with_view_id(&self, view_id: &str) -> Option<DatabaseMeta> {
    let all = self.get_all_database_meta();
    all
      .into_iter()
      .find(|record| record.linked_views.iter().any(|id| id == view_id))
  }

  pub fn get_database_meta(&self, database_id: &str) -> Option<DatabaseMeta> {
    let all = self.get_all_database_meta();
    all
      .into_iter()
      .find(|record| record.database_id == database_id)
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    CollabType::WorkspaceDatabase.validate_require_data(&self.collab)?;
    Ok(())
  }

  pub fn encode_collab_v1(&self) -> Result<EncodedCollab, DatabaseError> {
    self.validate()?;
    self
      .collab
      .encode_collab_v1(|_collab| Ok::<_, DatabaseError>(()))
  }
}

/// [DatabaseMeta] is a structure used to manage and track the metadata of views associated with a particular database.
/// It's primarily used to maintain a record of all views that are attached to a database, facilitating easier tracking and management.
///
#[derive(Clone, Debug)]
pub struct DatabaseMeta {
  pub database_id: String,
  pub created_at: i64,
  /// The first view should be the inline view
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

impl Borrow<Collab> for WorkspaceDatabase {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for WorkspaceDatabase {
  #[inline]
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
  }
}

pub struct WorkspaceDatabaseBody {
  array_ref: ArrayRef,
}

impl WorkspaceDatabaseBody {
  pub fn open(collab: &mut Collab) -> Result<Self, DatabaseError> {
    let txn = collab.context.transact();
    let array_ref = collab
      .data
      .get_with_txn(&txn, WORKSPACE_DATABASES)
      .ok_or_else(|| DatabaseError::NoRequiredData(WORKSPACE_DATABASES.to_string()))?;
    Ok(Self { array_ref })
  }

  pub fn create(collab: &mut Collab) -> Self {
    let mut txn = collab.context.transact_mut();
    let array_ref = collab.data.get_or_init(&mut txn, WORKSPACE_DATABASES);
    drop(txn);
    Self { array_ref }
  }

  pub fn push_back(&self, txn: &mut TransactionMut, value: DatabaseMeta) -> MapRef {
    let map_ref: MapRef = self.array_ref.push_back(txn, MapPrelim::default());
    value.fill_map_ref(txn, &map_ref);
    map_ref
  }

  pub fn index_of_database<T: ReadTxn>(&self, txn: &T, database_id: &str) -> Option<u32> {
    self
      .array_ref
      .iter(txn)
      .position(|value| {
        database_id_from_value(txn, value)
          .map(|id| id == database_id)
          .unwrap_or(false)
      })
      .map(|index| index as u32)
  }

  pub fn get_all_meta<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseMeta> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| {
        let map_ref: MapRef = value.cast().ok()?;
        DatabaseMeta::from_map_ref(txn, &map_ref)
      })
      .collect()
  }

  pub fn contains_database<T: ReadTxn>(&self, txn: &T, database_id: &str) -> bool {
    self.array_ref.iter(txn).any(|value| {
      database_id_from_value(txn, value)
        .map(|id| id == database_id)
        .unwrap_or(false)
    })
  }

  pub fn add_database(&self, txn: &mut TransactionMut, database_id: &str, view_ids: Vec<String>) {
    let linked_views: HashSet<String> = view_ids.into_iter().collect();
    let record = DatabaseMeta {
      database_id: database_id.to_string(),
      created_at: timestamp(),
      linked_views: linked_views.into_iter().collect(),
    };
    self.push_back(txn, record);
  }

  pub fn batch_add_database(
    &mut self,
    txn: &mut TransactionMut,
    view_ids_by_database_id: HashMap<String, Vec<String>>,
  ) {
    for (database_id, view_ids) in view_ids_by_database_id {
      let linked_views: HashSet<String> = view_ids.into_iter().collect();
      let record = DatabaseMeta {
        database_id,
        created_at: timestamp(),
        linked_views: linked_views.into_iter().collect(),
      };
      self.push_back(txn, record);
    }
  }

  pub fn delete_database(&self, txn: &mut TransactionMut, database_id: &str) {
    if let Some(index) = self.index_of_database(txn, database_id) {
      self.array_ref.remove(txn, index);
    }
  }

  pub fn update_database(
    &mut self,
    txn: &mut TransactionMut,
    database_id: &str,
    mut f: impl FnMut(&mut DatabaseMeta),
  ) {
    let index = self.index_of_database(txn, database_id);

    if let Some(index) = index {
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
}
