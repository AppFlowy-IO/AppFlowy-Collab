use crate::fields::FieldType;
use crate::{impl_i64_update, impl_str_update};
use collab::preclude::{ArrayRefWrapper, MapRefTool, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};

pub struct FilterArray {
  array_ref: ArrayRefWrapper,
}

impl FilterArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Filter>) {
    for filter in others {
      let filter_map_ref = self.array_ref.insert_map_with_txn(txn);
      FilterBuilder::new(
        &filter.id,
        filter.field_id,
        filter.field_type,
        txn,
        filter_map_ref,
      )
      .update(|update| {
        update
          .set_condition(filter.condition)
          .set_content(filter.content);
      });
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Filter {
  pub id: String,
  pub field_id: String,
  pub field_type: FieldType,
  pub condition: i64,
  pub content: String,
}

const FILTER_ID: &str = "id";
const FIELD_ID: &str = "field_id";
const FIELD_TYPE: &str = "ty";
const FILTER_CONDITION: &str = "condition";
const FILTER_CONTENT: &str = "content";

pub struct FilterBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> FilterBuilder<'a, 'b> {
  pub fn new(
    id: &'a str,
    field_id: String,
    field_type: FieldType,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRefWrapper,
  ) -> Self {
    map_ref.insert_with_txn(txn, FILTER_ID, id);
    map_ref.insert_with_txn(txn, FIELD_ID, field_id);
    map_ref.insert_with_txn(txn, FIELD_TYPE, field_type);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(FilterUpdate),
  {
    let update = FilterUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct FilterUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> FilterUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_content, set_content_if_not_none, FILTER_CONTENT);
  impl_i64_update!(set_condition, set_condition_if_not_none, FILTER_CONDITION);

  pub fn done(self) -> Option<Filter> {
    filter_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn filter_from_map_ref<T: ReadTxn>(map_ref: &MapRefWrapper, txn: &T) -> Option<Filter> {
  let map_ref = MapRefTool(map_ref);
  let id = map_ref.get_str_with_txn(txn, FILTER_ID)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let condition = map_ref.get_i64_with_txn(txn, FILTER_CONDITION).unwrap_or(0);
  let content = map_ref
    .get_str_with_txn(txn, FILTER_CONTENT)
    .unwrap_or_default();
  let field_type = map_ref
    .get_i64_with_txn(txn, FIELD_TYPE)
    .map(|value| value.try_into().ok())??;

  Some(Filter {
    id,
    field_id,
    field_type,
    condition,
    content,
  })
}
