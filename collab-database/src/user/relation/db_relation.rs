use collab::core::collab::Path;
use collab::preclude::{Collab, MapRefWrapper};

use crate::user::relation::RowRelationMap;

pub struct DatabaseRelation {
  #[allow(dead_code)]
  inner: Collab,
  row_relation_map: RowRelationMap,
}

const ROW_RELATION_MAP: &str = "row_relations";
impl DatabaseRelation {
  pub fn new(collab: Collab) -> DatabaseRelation {
    let row_relation_map = {
      let txn = collab.transact();
      collab.get_map_with_txn(&txn, vec![ROW_RELATION_MAP])
    };

    let relation_map = match row_relation_map {
      None => collab.with_transact_mut(|txn| collab.create_map_with_txn(txn, ROW_RELATION_MAP)),
      Some(row_relation_map) => row_relation_map,
    };

    Self {
      inner: collab,
      row_relation_map: RowRelationMap::from_map_ref(relation_map),
    }
  }

  pub fn row_relations(&self) -> &RowRelationMap {
    &self.row_relation_map
  }
}
