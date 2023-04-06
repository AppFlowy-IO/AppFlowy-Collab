use collab::core::array_wrapper::ArrayRefExtension;
use collab::core::lib0_any_ext::{AnyMap, AnyMapBuilder};
use collab::preclude::{Array, ArrayRef, ReadTxn, TransactionMut};

pub struct FilterArray {
  array_ref: ArrayRef,
}

impl FilterArray {
  pub fn new(array_ref: ArrayRef) -> Self {
    Self { array_ref }
  }

  pub fn extends_with_txn(&self, txn: &mut TransactionMut, others: Vec<FilterMap>) {
    let array_ref = ArrayRefExtension(&self.array_ref);
    for filter in others {
      let filter_map_ref = array_ref.insert_map_with_txn(txn);
      filter.fill_map_ref(txn, filter_map_ref);
    }
  }

  pub fn get_filters_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<FilterMap> {
    self
      .array_ref
      .iter(txn)
      .flat_map(|v| FilterMap::from_value(txn, v))
      .collect::<Vec<FilterMap>>()
  }
}

pub type FilterMap = AnyMap;
pub type FilterMapBuilder = AnyMapBuilder;
