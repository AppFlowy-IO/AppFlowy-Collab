use collab::preclude::{
  Array, Doc, Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, Transact, Transaction,
  TransactionMut, YrsValue,
};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

pub struct DatabaseRelation {
  doc: Doc,
  relations: RelationMap,
}

impl DatabaseRelation {
  pub fn new(doc: Doc) -> DatabaseRelation {
    let relations = RelationMap::from_doc(doc.clone());
    Self { doc, relations }
  }

  pub fn relations(&self) -> &RelationMap {
    &self.relations
  }
}

pub struct RelationMap {
  doc: Doc,
  map_ref: MapRef,
}

impl RelationMap {
  pub fn from_doc(doc: Doc) -> Self {
    let map_ref = doc.get_or_insert_map("row_connections");
    Self { doc, map_ref }
  }

  pub fn get_linking_rows(&self, database_id: &str, _row_id: &str) -> Vec<LinkingRow> {
    let txn = self.doc.transact();
    match self.map_ref.get_map_with_txn(&txn, database_id) {
      None => vec![],
      Some(_map_ref) => {
        vec![]
      },
    }
  }

  pub fn get_link_by_rows(&self, _database_id: &str, _row_id: &str) -> Vec<LinkedByRow> {
    todo!()
  }

  pub fn transact(&self) -> Transaction {
    self.doc.transact()
  }

  pub fn with_transact_mut<F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut TransactionMut) -> T,
  {
    let mut txn = self.doc.transact_mut();
    f(&mut txn)
  }
}

impl Deref for RelationMap {
  type Target = MapRef;

  fn deref(&self) -> &Self::Target {
    &self.map_ref
  }
}

impl DerefMut for RelationMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.map_ref
  }
}

impl MapRefExtension for RelationMap {
  fn map_ref(&self) -> &MapRef {
    &self.map_ref
  }
}

pub struct RowConnection {
  linking_database_id: String,
  linked_by_database_id: String,
  row_relations: HashMap<String, RowRelation>,
}

const LINKING_DB_ID: &str = "linking_db";
const LINKED_BY_DB_ID: &str = "linked_db";
const ROW_RELATIONS: &str = "row_relations";

impl RowConnection {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<Self> {
    let linking_database_id = map_ref.get_str_with_txn(txn, LINKING_DB_ID)?;
    let linked_by_database_id = map_ref.get_str_with_txn(txn, LINKED_BY_DB_ID)?;
    let row_relations_map = map_ref.get_map_with_txn(txn, ROW_RELATIONS)?;
    let row_relations = row_relations_map
      .iter(txn)
      .flat_map(|(k, v)| {
        let map_ref = v.to_ymap()?;
        let row_relations = RowRelation::from_map_ref(txn, &map_ref)?;
        Some((k.to_string(), row_relations))
      })
      .collect::<HashMap<String, RowRelation>>();

    Some(Self {
      linking_database_id,
      linked_by_database_id,
      row_relations,
    })
  }
}

pub struct RowRelation {
  row_id: String,
  linking_rows: Vec<LinkingRow>,
  linked_by_rows: Vec<LinkedByRow>,
}

const ROW_ID: &str = "row_id";
const LINKING_ROWS: &str = "linking_rows";
const LINKED_BY_ROWS: &str = "linked_by_rows";

impl RowRelation {
  pub fn from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<RowRelation> {
    let row_id = map_ref.get_str_with_txn(txn, ROW_ID)?;
    let linking_rows = map_ref
      .get_array_ref_with_txn(txn, LINKING_ROWS)?
      .iter(txn)
      .flat_map(|value| LinkingRow::from_yrs_value(txn, value))
      .collect::<Vec<_>>();
    let linked_by_rows = map_ref
      .get_array_ref_with_txn(txn, LINKED_BY_ROWS)?
      .iter(txn)
      .flat_map(|value| LinkedByRow::from_yrs_value(txn, value))
      .collect::<Vec<_>>();
    Some(Self {
      row_id,
      linking_rows,
      linked_by_rows,
    })
  }
}

pub struct LinkingRow {
  pub row_id: String,
  pub content: String,
}

impl LinkingRow {
  pub fn from_yrs_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<LinkingRow> {
    let map_ref = value.to_ymap()?;
    let row_id = map_ref.get_str_with_txn(txn, "row_id")?;
    let content = map_ref.get_str_with_txn(txn, "content")?;
    Some(Self { row_id, content })
  }
}

pub struct LinkedByRow {
  pub row_id: String,
}

impl LinkedByRow {
  pub fn from_yrs_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<LinkedByRow> {
    let map_ref = value.to_ymap()?;
    let row_id = map_ref.get_str_with_txn(txn, "row_id")?;
    Some(Self { row_id })
  }
}
