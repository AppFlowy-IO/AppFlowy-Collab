use collab::core::any_array::ArrayMap;
use collab::core::any_map::{AnyMap, AnyMapBuilder};

pub type SortArray = ArrayMap;
pub type SortMap = AnyMap;
pub type SortMapBuilder = AnyMapBuilder;

// pub struct SortBuilder<'a, 'b> {
//   id: &'a str,
//   map_ref: MapRef,
//   txn: &'a mut TransactionMut<'b>,
// }
//
// impl<'a, 'b> SortBuilder<'a, 'b> {
//   pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: MapRef) -> Self {
//     map_ref.insert_with_txn(txn, SORT_ID, id);
//     Self { id, map_ref, txn }
//   }
//
//   pub fn update<F>(self, f: F) -> Self
//   where
//     F: FnOnce(SortUpdate),
//   {
//     let update = SortUpdate::new(self.id, self.txn, &self.map_ref);
//     f(update);
//     self
//   }
//   pub fn done(self) {}
// }

// pub struct SortUpdate<'a, 'b> {
//   #[allow(dead_code)]
//   id: &'a str,
//   map_ref: &'a MapRef,
//   txn: &'a mut TransactionMut<'b>,
// }
//
// impl<'a, 'b> SortUpdate<'a, 'b> {
//   pub fn new(id: &'a str, txn: &'a mut TransactionMut<'b>, map_ref: &'a MapRef) -> Self {
//     Self { id, map_ref, txn }
//   }
//
//   impl_str_update!(set_field_id, set_field_id_if_not_none, FIELD_ID);
//   impl_any_update!(
//     set_condition,
//     set_condition_if_not_none,
//     SORT_CONDITION,
//     SortCondition
//   );
//   impl_i64_update!(set_field_type, set_field_type_if_not_none, FIELD_TYPE);
//
//   pub fn done(self) -> Option<Sort> {
//     sort_from_map_ref(self.map_ref, self.txn)
//   }
// }
