use anyhow::bail;
use collab::preclude::{
  Array, ArrayRefWrapper, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};

pub struct DatabaseArray {
  array_ref: ArrayRefWrapper,
}

impl DatabaseArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn add_database(&self, database_id: &str, name: &str) {
    self.array_ref.with_transact_mut(|txn| {
      let record = DatabaseRecord {
        database_id: database_id.to_string(),
        name: name.to_string(),
        created_at: chrono::Utc::now().timestamp(),
      };
      let map_ref = self.array_ref.insert_map_with_txn(txn);
      record.fill_map_ref(txn, map_ref);
    });
  }

  pub fn delete_database(&self, database_id: &str) {
    self.array_ref.with_transact_mut(|txn| {
      if let Some(index) =
        self
          .array_ref
          .iter(txn)
          .position(|value| match database_id_from_value(txn, value) {
            None => false,
            Some(id) => database_id == id,
          })
      {
        self.array_ref.remove(txn, index as u32);
      }
    })
  }

  pub fn get_all_databases(&self) -> Vec<DatabaseRecord> {
    self
      .array_ref
      .with_transact_mut(|txn| self.get_all_databases_with_txn(txn))
  }

  pub fn get_all_databases_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<DatabaseRecord> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|value| {
        let map_ref = value.to_ymap()?;
        (txn, &map_ref).try_into().ok()
      })
      .collect()
  }
}

pub struct DatabaseRecord {
  pub database_id: String,
  pub name: String,
  pub created_at: i64,
}

const DATABASE_RECORD_ID: &str = "database_id";
const DATABASE_RECORD_NAME: &str = "name";
const DATABASE_RECORD_CREATED_AT: &str = "created_at";

impl DatabaseRecord {
  fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: MapRefWrapper) {
    map_ref.insert_with_txn(txn, DATABASE_RECORD_ID, self.database_id);
    map_ref.insert_with_txn(txn, DATABASE_RECORD_NAME, self.name);
    map_ref.insert_with_txn(txn, DATABASE_RECORD_CREATED_AT, self.created_at);
  }
}

impl<T: ReadTxn> TryFrom<(&'_ T, &MapRef)> for DatabaseRecord {
  type Error = anyhow::Error;

  fn try_from(params: (&'_ T, &MapRef)) -> Result<Self, Self::Error> {
    let (txn, map_ref) = params;
    let f = || {
      let id = map_ref.get_str_with_txn(txn, DATABASE_RECORD_ID)?;
      let name = map_ref.get_str_with_txn(txn, DATABASE_RECORD_NAME)?;
      let created_at = map_ref.get_i64_with_txn(txn, DATABASE_RECORD_CREATED_AT)?;
      Some((id, name, created_at))
    };
    match f() {
      None => bail!("Invalid database record"),
      Some((id, name, created_at)) => Ok(DatabaseRecord {
        database_id: id,
        name,
        created_at,
      }),
    }
  }
}

fn database_id_from_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<String> {
  if let YrsValue::YMap(map_ref) = value {
    map_ref.get_str_with_txn(txn, DATABASE_RECORD_ID)
  } else {
    None
  }
}
