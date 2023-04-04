use crate::{impl_i64_update, impl_str_update};
use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  Array, ArrayRef, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};

pub struct FilterArray {
  array_ref: ArrayRef,
}

impl FilterArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Filter>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for filter in others {
      let filter_map_ref = array_ref.insert_map_with_txn(txn);
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

  pub fn get_filters_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Filter> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| filter_from_value(v, txn))
      .collect::<Vec<Filter>>()
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Filter {
  pub id: String,
  pub field_id: String,
  pub field_type: i64,
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
  map_ref: MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> FilterBuilder<'a, 'b> {
  pub fn new(
    id: &'a str,
    field_id: String,
    field_type: i64,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRef,
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

pub struct FilterUpdate<'a, 'b> {
  #[allow(dead_code)]
  id: &'a str,
  map_ref: &'a MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> FilterUpdate<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_content, set_content_if_not_none, FILTER_CONTENT);
  impl_i64_update!(set_condition, set_condition_if_not_none, FILTER_CONDITION);

  pub fn done(self) -> Option<Filter> {
    filter_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn filter_from_value<T: ReadTxn>(value: YrsValue, txn: &T) -> Option<Filter> {
  if let YrsValue::YMap(map_ref) = value {
    filter_from_map_ref(&map_ref, txn)
  } else {
    None
  }
}

pub fn filter_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Filter> {
  let id = map_ref.get_str_with_txn(txn, FILTER_ID)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let condition = map_ref.get_i64_with_txn(txn, FILTER_CONDITION).unwrap_or(0);
  let content = map_ref
    .get_str_with_txn(txn, FILTER_CONTENT)
    .unwrap_or_default();
  let field_type = map_ref
    .get_i64_with_txn(txn, FIELD_TYPE)?;

  Some(Filter {
    id,
    field_id,
    field_type,
    condition,
    content,
  })
}
