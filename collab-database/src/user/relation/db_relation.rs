use crate::user::relation::RowRelationMap;
use collab::preclude::Collab;

pub struct DatabaseRelation {
  #[allow(dead_code)]
  inner: Collab,
  row_relation_map: RowRelationMap,
}

const ROW_RELATION_MAP: &str = "row_relations";
impl DatabaseRelation {
  pub fn new(collab: Collab) -> DatabaseRelation {
    let row_relation_map = collab.with_transact_mut(|txn| {
      let relation_map = collab
        .get_map_with_txn(txn, vec![ROW_RELATION_MAP])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, ROW_RELATION_MAP));

      RowRelationMap::from_map_ref(relation_map)
    });

    Self {
      inner: collab,
      row_relation_map,
    }
  }

  pub fn row_relations(&self) -> &RowRelationMap {
    &self.row_relation_map
  }
}
