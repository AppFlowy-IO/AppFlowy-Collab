use std::collections::HashMap;

use collab::preclude::{
  Array, ArrayRef, Map, MapExt, MapPrelim, MapRef, ReadTxn, TransactionMut, YrsValue,
};

#[derive(Debug, Clone)]
pub struct RowRelation {
  pub linking_database_id: String,
  pub linked_by_database_id: String,
  pub row_connections: HashMap<String, RowConnection>,
}

const LINKING_DB_ID: &str = "linking_db";
const LINKED_BY_DB_ID: &str = "linked_db";
const ROW_CONNECTIONS: &str = "row_connections";

impl RowRelation {
  pub fn id(&self) -> String {
    format!(
      "{}-{}",
      self.linking_database_id, self.linked_by_database_id
    )
  }
}

pub struct RowRelationBuilder<'a, 'b> {
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowRelationBuilder<'a, 'b> {
  pub fn new(
    linking_database_id: &str,
    linked_by_database_id: &str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRef,
  ) -> Self {
    map_ref.insert(txn, LINKING_DB_ID, linking_database_id);
    map_ref.insert(txn, LINKED_BY_DB_ID, linked_by_database_id);
    Self { map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(RowRelationUpdate),
  {
    let update = RowRelationUpdate::new(self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct RowRelationUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowRelationUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn set_row_connections(self, connections: HashMap<String, RowConnection>) -> Self {
    connections.into_iter().for_each(|(k, v)| {
      let map_ref: MapRef = self.map_ref.get_or_init(self.txn, k);
      RowConnectionBuilder::new(&v.row_id, self.txn, map_ref).update(|update| {
        update
          .set_linking_rows(v.linking_rows)
          .set_linked_by_rows(v.linked_by_rows);
      });
    });
    self
  }

  pub fn done(self) -> Option<RowRelation> {
    row_relation_from_map_ref(self.txn, self.map_ref)
  }
}

pub fn row_relation_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<RowRelation> {
  let linking_database_id: String = map_ref.get_with_txn(txn, LINKING_DB_ID)?;
  let linked_by_database_id: String = map_ref.get_with_txn(txn, LINKED_BY_DB_ID)?;
  let row_connections = map_ref
    .get_with_txn::<_, MapRef>(txn, ROW_CONNECTIONS)
    .map(|map_ref| {
      map_ref
        .iter(txn)
        .flat_map(|(k, v)| {
          let map_ref = v.cast().ok()?;
          let row_connection = row_connection_from_map_ref(txn, &map_ref)?;
          Some((k.to_string(), row_connection))
        })
        .collect::<HashMap<String, RowConnection>>()
    })
    .unwrap_or_default();

  Some(RowRelation {
    linking_database_id,
    linked_by_database_id,
    row_connections,
  })
}

#[derive(Debug, Clone)]
pub struct RowConnection {
  row_id: String,
  linking_rows: Vec<LinkingRow>,
  linked_by_rows: Vec<LinkedByRow>,
}

const ROW_ID: &str = "row_id";
const LINKING_ROWS: &str = "linking_rows";
const LINKED_BY_ROWS: &str = "linked_by_rows";

pub struct RowConnectionBuilder<'a, 'b> {
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowConnectionBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
    map_ref.insert(txn, ROW_ID, id);
    Self { map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(RowConnectionUpdate),
  {
    let update = RowConnectionUpdate::new(self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct RowConnectionUpdate<'a, 'b> {
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> RowConnectionUpdate<'a, 'b> {
  pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { map_ref, txn }
  }

  pub fn set_linking_rows(self, rows: Vec<LinkingRow>) -> Self {
    let array_ref: ArrayRef = self.map_ref.get_or_init(self.txn, LINKING_ROWS);
    for row in rows {
      let map_ref = array_ref.push_back(self.txn, MapPrelim::default());
      row.fill_map_with_txn(self.txn, map_ref);
    }
    self
  }

  pub fn set_linked_by_rows(self, rows: Vec<LinkedByRow>) -> Self {
    let array_ref: ArrayRef = self.map_ref.get_or_init(self.txn, LINKED_BY_ROWS);

    for row in rows {
      let map_ref = array_ref.push_back(self.txn, MapPrelim::default());
      row.fill_map_with_txn(self.txn, map_ref);
    }
    self
  }

  pub fn done(self) -> Option<RowConnection> {
    row_connection_from_map_ref(self.txn, self.map_ref)
  }
}

pub fn row_connection_from_map_ref<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Option<RowConnection> {
  let row_id: String = map_ref.get_with_txn(txn, ROW_ID)?;
  let linking_rows = map_ref
    .get_with_txn::<_, ArrayRef>(txn, LINKING_ROWS)?
    .iter(txn)
    .flat_map(|value| LinkingRow::from_yrs_value(txn, value))
    .collect::<Vec<_>>();
  let linked_by_rows = map_ref
    .get_with_txn::<_, ArrayRef>(txn, LINKED_BY_ROWS)?
    .iter(txn)
    .flat_map(|value| LinkedByRow::from_yrs_value(txn, value))
    .collect::<Vec<_>>();
  Some(RowConnection {
    row_id,
    linking_rows,
    linked_by_rows,
  })
}

#[derive(Debug, Clone)]
pub struct LinkingRow {
  pub row_id: String,
  pub content: String,
}

impl LinkingRow {
  pub fn from_yrs_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<LinkingRow> {
    let map_ref: MapRef = value.cast().ok()?;
    let row_id: String = map_ref.get_with_txn(txn, "row_id")?;
    let content: String = map_ref.get_with_txn(txn, "content")?;
    Some(Self { row_id, content })
  }

  pub fn fill_map_with_txn(self, txn: &mut TransactionMut, map_ref: MapRef) {
    map_ref.insert(txn, "row_id", self.row_id);
    map_ref.insert(txn, "content", self.content);
  }
}

#[derive(Debug, Clone)]
pub struct LinkedByRow {
  pub row_id: String,
}

impl LinkedByRow {
  pub fn from_yrs_value<T: ReadTxn>(txn: &T, value: YrsValue) -> Option<LinkedByRow> {
    let map_ref: MapRef = value.cast().ok()?;
    Some(Self {
      row_id: map_ref.get_with_txn(txn, "row_id")?,
    })
  }

  pub fn fill_map_with_txn(self, txn: &mut TransactionMut, map_ref: MapRef) {
    map_ref.insert(txn, "row_id", self.row_id);
  }
}
