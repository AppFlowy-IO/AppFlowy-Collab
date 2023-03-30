use crate::fields::FieldType;
use crate::{impl_any_update, impl_str_update};
use anyhow::bail;
use collab::preclude::{
  lib0Any, ArrayRefWrapper, MapRef, MapRefTool, MapRefWrapper, ReadTxn, TransactionMut,
};
use serde::{Deserialize, Serialize};
use serde_repr::*;

pub struct SortArray {
  array_ref: ArrayRefWrapper,
}

impl SortArray {
  pub fn new(array_ref: ArrayRefWrapper) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<Sort>) {
    for sort in others {
      let sort_map_ref = self.array_ref.insert_map_with_txn(txn);
      SortBuilder::new(&sort.id, txn, sort_map_ref).update(|update| {
        update
          .set_condition(sort.condition)
          .set_field_type(sort.field_type)
          .set_field_id(sort.field_id);
      });
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct Sort {
  pub id: String,
  pub field_id: String,
  pub field_type: FieldType,
  pub condition: SortCondition,
}

const SORT_ID: &str = "id";
const FIELD_ID: &str = "field_id";
const FIELD_TYPE: &str = "ty";
const SORT_CONDITION: &str = "condition";
pub struct SortBuilder<'a, 'b> {
  id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> SortBuilder<'a, 'b> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
    map_ref.insert_with_txn(txn, SORT_ID, id);
    Self { id, map_ref, txn }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(SortUpdate),
  {
    let update = SortUpdate::new(self.id, self.txn, &self.map_ref);
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct SortUpdate<'a, 'b, 'c> {
  id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> SortUpdate<'a, 'b, 'c> {
  pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
    Self { id, map_ref, txn }
  }

  impl_str_update!(set_field_id, set_field_id_if_not_none, FIELD_ID);
  impl_any_update!(
    set_condition,
    set_condition_if_not_none,
    SORT_CONDITION,
    SortCondition
  );
  impl_any_update!(
    set_field_type,
    set_field_type_if_not_none,
    FIELD_TYPE,
    FieldType
  );

  pub fn done(self) -> Option<Sort> {
    sort_from_map_ref(self.map_ref, self.txn)
  }
}

pub fn sort_from_map_ref<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<Sort> {
  let map_ref = MapRefTool(map_ref);
  let id = map_ref.get_str_with_txn(txn, SORT_ID)?;
  let field_id = map_ref.get_str_with_txn(txn, FIELD_ID)?;
  let field_type = map_ref
    .get_i64_with_txn(txn, FIELD_TYPE)
    .map(|value| value.try_into().ok())??;

  let condition = map_ref
    .get_i64_with_txn(txn, SORT_CONDITION)
    .map(|value| value.try_into().ok())??;

  Some(Sort {
    id,
    field_id,
    field_type,
    condition,
  })
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, Clone, Debug)]
#[repr(u8)]
pub enum SortCondition {
  Ascending = 0,
  Descending = 1,
}

impl Default for SortCondition {
  fn default() -> Self {
    Self::Ascending
  }
}

impl TryFrom<i64> for SortCondition {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(SortCondition::Ascending),
      1 => Ok(SortCondition::Descending),
      _ => bail!("Unknown field type {}", value),
    }
  }
}

impl From<SortCondition> for lib0Any {
  fn from(condition: SortCondition) -> Self {
    lib0Any::BigInt(condition as i64)
  }
}
