use collab::lock::Mutex;
use collab::preclude::{Collab, Map};
use std::sync::Arc;

use crate::workspace_database::relation::RowRelationMap;

pub struct DatabaseRelation {
  #[allow(dead_code)]
  inner: Arc<Mutex<Collab>>,
  row_relation_map: RowRelationMap,
}

const ROW_RELATION_MAP: &str = "row_relations";
impl DatabaseRelation {
  pub fn new(collab: Arc<Mutex<Collab>>) -> DatabaseRelation {
    let relation_map = {
      let mut lock = collab.blocking_lock(); //FIXME: was that safe before?
      let collab = &mut *lock;
      let mut txn = collab.context.transact_mut();
      collab.data.get_or_init(&mut txn, ROW_RELATION_MAP)
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
