use std::sync::Arc;

use collab::core::collab::MutexCollab;

use crate::user::relation::RowRelationMap;

pub struct DatabaseRelation {
  #[allow(dead_code)]
  inner: Arc<MutexCollab>,
  row_relation_map: RowRelationMap,
}

const ROW_RELATION_MAP: &str = "row_relations";
impl DatabaseRelation {
  pub fn new(collab: Arc<MutexCollab>) -> DatabaseRelation {
    let collab_guard = collab.lock();
    let row_relation_map = {
      let txn = collab_guard.transact();
      collab_guard.get_map_with_txn(&txn, vec![ROW_RELATION_MAP])
    };

    let relation_map = match row_relation_map {
      None => collab_guard
        .with_origin_transact_mut(|txn| collab_guard.insert_map_with_txn(txn, ROW_RELATION_MAP)),
      Some(row_relation_map) => row_relation_map,
    };

    drop(collab_guard);

    Self {
      inner: collab,
      row_relation_map: RowRelationMap::from_map_ref(relation_map),
    }
  }

  pub fn row_relations(&self) -> &RowRelationMap {
    &self.row_relation_map
  }
}
