use crate::views::{
  view_from_map_ref, view_from_value, view_id_from_map_ref, View, ViewBuilder, ViewUpdate,
};
use collab::preclude::{
  Map, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut, YrsValue,
};

pub struct ViewMap {
  container: MapRefWrapper,
}

impl ViewMap {
  pub fn new(container: MapRefWrapper) -> Self {
    // let field_order = FieldOrderArray::new(field_order);
    Self { container }
  }

  pub fn insert_view(&self, view: View) {
    self
      .container
      .with_transact_mut(|txn| self.insert_view_with_txn(txn, view))
  }

  pub fn insert_view_with_txn(&self, txn: &mut TransactionMut, view: View) {
    let map_ref = self.container.insert_map_with_txn(txn, &view.id);
    ViewBuilder::new(&view.id, txn, map_ref).update(|update| {
      update
        .set_name(view.name)
        .set_database_id(view.database_id)
        .set_layout_settings(view.layout_settings)
        .set_layout_type(view.layout)
        .set_filter(view.filters)
        .set_groups(view.groups)
        .set_sorts(view.sorts)
        .set_field_orders(view.field_orders)
        .set_row_orders(view.row_orders);
    });
  }

  pub fn get_view(&self, view_id: &str) -> Option<View> {
    let txn = self.container.transact();
    self.get_view_with_txn(&txn, view_id)
  }

  pub fn get_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<View> {
    let map_ref = self.container.get_map_with_txn(txn, view_id)?;
    view_from_map_ref(&map_ref, txn)
  }

  pub fn get_all_views_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<View> {
    self
      .container
      .iter(txn)
      .flat_map(|(k, v)| view_from_value(v, txn))
      .collect::<Vec<_>>()
  }

  pub fn update_all_views_with_txn<F>(&self, txn: &mut TransactionMut, f: F)
  where
    F: Fn(ViewUpdate),
  {
    let map_refs = self
      .container
      .iter(txn)
      .flat_map(|(k, v)| v.to_ymap())
      .collect::<Vec<MapRef>>();

    for map_ref in map_refs {
      if let Some(view_id) = view_id_from_map_ref(&map_ref, txn) {
        let map_ref_ext = MapRefExtension(&map_ref);
        let update = ViewUpdate::new(&view_id, txn, map_ref_ext);
        f(update)
      }
    }
  }
}
