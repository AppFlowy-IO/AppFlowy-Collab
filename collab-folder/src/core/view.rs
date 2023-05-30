use crate::core::{subscribe_view_change, RepeatedView, ViewIdentifier, ViewRelations};
use crate::{impl_any_update, impl_i64_update, impl_option_str_update, impl_str_update};
use anyhow::bail;

use crate::core::folder_observe::{ViewChange, ViewChangeSender};
use collab::preclude::{
  lib0Any, DeepEventsSubscription, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use serde::{Deserialize, Serialize};
use serde_repr::*;
use std::rc::Rc;

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_BID: &str = "bid";
const VIEW_DESC: &str = "desc";
const VIEW_DATABASE_ID: &str = "database_id";
const VIEW_LAYOUT: &str = "layout";
const VIEW_CREATE_AT: &str = "created_at";

pub struct ViewsMap {
  container: MapRefWrapper,
  #[allow(dead_code)]
  subscription: DeepEventsSubscription,
  change_tx: ViewChangeSender,
  view_relations: Rc<ViewRelations>,
}

impl ViewsMap {
  pub fn new(
    mut root: MapRefWrapper,
    change_tx: ViewChangeSender,
    views_relation: Rc<ViewRelations>,
  ) -> ViewsMap {
    let subscription = subscribe_view_change(&mut root, change_tx.clone(), views_relation.clone());
    Self {
      container: root,
      subscription,
      change_tx,
      view_relations: views_relation,
    }
  }

  pub fn get_views_belong_to(&self, bid: &str) -> Vec<View> {
    let txn = self.container.transact();
    self.get_views_belong_to_with_txn(&txn, bid)
  }

  pub fn get_views_belong_to_with_txn<T: ReadTxn>(&self, txn: &T, bid: &str) -> Vec<View> {
    match self.get_view_with_txn(txn, bid) {
      Some(root_view) => root_view
        .children
        .iter()
        .flat_map(|be| {
          self
            .container
            .get_map_with_txn(txn, &be.id)
            .and_then(|map| view_from_map_ref(&map, txn, &self.view_relations))
        })
        .collect::<Vec<View>>(),
      None => vec![],
    }
  }

  pub fn get_views<T: AsRef<str>>(&self, view_ids: &[T]) -> Vec<View> {
    let txn = self.container.transact();
    self.get_views_with_txn(&txn, view_ids)
  }

  pub fn get_views_with_txn<T: ReadTxn, V: AsRef<str>>(
    &self,
    txn: &T,
    view_ids: &[V],
  ) -> Vec<View> {
    view_ids
      .iter()
      .flat_map(|view_id| self.get_view_with_txn(txn, view_id.as_ref()))
      .collect::<Vec<_>>()
  }

  pub fn get_view(&self, view_id: &str) -> Option<View> {
    let txn = self.container.transact();
    self.get_view_with_txn(&txn, view_id)
  }

  pub fn get_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<View> {
    let map_ref = self.container.get_map_with_txn(txn, view_id)?;
    view_from_map_ref(&map_ref, txn, &self.view_relations)
  }

  pub fn get_view_name_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<String> {
    let map_ref = self.container.get_map_with_txn(txn, view_id)?;
    map_ref.get_str_with_txn(txn, VIEW_NAME)
  }

  pub(crate) fn insert_view(&self, view: View) {
    self
      .container
      .with_transact_mut(|txn| self.insert_view_with_txn(txn, view));
  }

  pub(crate) fn insert_view_with_txn(&self, txn: &mut TransactionMut, view: View) {
    if let Some(parent_map_ref) = self.container.get_map_with_txn(txn, &view.bid) {
      let belonging = ViewIdentifier {
        id: view.id.clone(),
      };
      ViewUpdate::new(&view.bid, txn, &parent_map_ref, self.view_relations.clone())
        .add_belonging(vec![belonging])
        .done();
    }

    let map_ref = self.container.insert_map_with_txn(txn, &view.id);
    ViewBuilder::new(&view.id, txn, map_ref, self.view_relations.clone())
      .update(|update| {
        update
          .set_name(view.name)
          .set_bid(view.bid)
          .set_desc(view.desc)
          .set_layout(view.layout)
          .set_created_at(view.created_at)
          .set_children(view.children)
          .done();
      })
      .done();
  }

  pub fn delete_views<T: AsRef<str>>(&self, view_ids: Vec<T>) {
    self
      .container
      .with_transact_mut(|txn| self.delete_views_with_txn(txn, view_ids));
  }

  pub fn delete_views_with_txn<T: AsRef<str>>(&self, txn: &mut TransactionMut, view_ids: Vec<T>) {
    // Get the view before deleting.
    let views = view_ids
      .iter()
      .flat_map(|view_id| self.get_view_with_txn(txn, view_id.as_ref()))
      .collect::<Vec<View>>();

    view_ids.iter().for_each(|view_id| {
      self.container.delete_with_txn(txn, view_id.as_ref());
    });

    let _ = self.change_tx.send(ViewChange::DidDeleteView { views });
  }

  pub fn update_view<F>(&self, view_id: &str, f: F) -> Option<View>
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_map_with_txn(txn, view_id)?;
      let update = ViewUpdate::new(view_id, txn, &map_ref, self.view_relations.clone());
      f(update)
    })
  }
}

pub(crate) fn view_from_map_ref<T: ReadTxn>(
  map_ref: &MapRef,
  txn: &T,
  belonging_map: &Rc<ViewRelations>,
) -> Option<View> {
  let bid = map_ref.get_str_with_txn(txn, VIEW_BID)?;
  let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
  let name = map_ref.get_str_with_txn(txn, VIEW_NAME).unwrap_or_default();
  let desc = map_ref.get_str_with_txn(txn, VIEW_DESC).unwrap_or_default();
  let created_at = map_ref
    .get_i64_with_txn(txn, VIEW_CREATE_AT)
    .unwrap_or_default();
  let layout = map_ref
    .get_i64_with_txn(txn, VIEW_LAYOUT)
    .map(|value| value.try_into().ok())??;

  let belongings = belonging_map
    .get_children_with_txn(txn, &id)
    .map(|array| array.get_children_with_txn(txn))
    .unwrap_or_default();

  Some(View {
    id,
    bid,
    name,
    desc,
    children: belongings,
    created_at,
    layout,
  })
}

pub struct ViewBuilder<'a, 'b> {
  view_id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  belongings: Rc<ViewRelations>,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
  pub fn new(
    view_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRefWrapper,
    belongings: Rc<ViewRelations>,
  ) -> Self {
    map_ref.insert_str_with_txn(txn, VIEW_ID, view_id);
    Self {
      view_id,
      map_ref,
      txn,
      belongings,
    }
  }

  pub fn update<F>(self, f: F) -> Self
  where
    F: FnOnce(ViewUpdate),
  {
    let update = ViewUpdate::new(
      self.view_id,
      self.txn,
      &self.map_ref,
      self.belongings.clone(),
    );
    f(update);
    self
  }
  pub fn done(self) {}
}

pub struct ViewUpdate<'a, 'b, 'c> {
  view_id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  children_map: Rc<ViewRelations>,
}

impl<'a, 'b, 'c> ViewUpdate<'a, 'b, 'c> {
  impl_str_update!(set_name, set_name_if_not_none, VIEW_NAME);
  impl_str_update!(set_bid, set_bid_if_not_none, VIEW_BID);
  impl_option_str_update!(
    set_database_id,
    set_database_id_if_not_none,
    VIEW_DATABASE_ID
  );
  impl_str_update!(set_desc, set_desc_if_not_none, VIEW_DESC);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, VIEW_CREATE_AT);
  impl_any_update!(set_layout, set_layout_if_not_none, VIEW_LAYOUT, ViewLayout);

  pub fn new(
    view_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: &'c MapRefWrapper,
    children_map: Rc<ViewRelations>,
  ) -> Self {
    Self {
      view_id,
      map_ref,
      txn,
      children_map,
    }
  }

  pub fn set_children(self, children: RepeatedView) -> Self {
    let array = self
      .children_map
      .get_or_create_children_with_txn(self.txn, self.view_id);
    array.add_children_with_txn(self.txn, children.into_inner());

    self
  }

  pub fn add_belonging(self, belongings: Vec<ViewIdentifier>) -> Self {
    self
      .children_map
      .add_children(self.txn, self.view_id, belongings);
    self
  }

  pub fn done(self) -> Option<View> {
    view_from_map_ref(self.map_ref, self.txn, &self.children_map)
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct View {
  pub id: String,
  // bid short for belong to id
  pub bid: String,
  pub name: String,
  pub desc: String,
  pub children: RepeatedView,
  pub created_at: i64,
  pub layout: ViewLayout,
}

#[derive(Eq, PartialEq, Debug, Hash, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ViewLayout {
  Document = 0,
  Grid = 1,
  Board = 2,
  Calendar = 3,
}

impl ViewLayout {
  pub fn is_database(&self) -> bool {
    matches!(
      self,
      ViewLayout::Grid | ViewLayout::Board | ViewLayout::Calendar
    )
  }
}

impl TryFrom<i64> for ViewLayout {
  type Error = anyhow::Error;

  fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
    match value {
      0 => Ok(ViewLayout::Document),
      1 => Ok(ViewLayout::Grid),
      2 => Ok(ViewLayout::Board),
      3 => Ok(ViewLayout::Calendar),
      _ => bail!("Unknown layout {}", value),
    }
  }
}

impl From<ViewLayout> for lib0Any {
  fn from(layout: ViewLayout) -> Self {
    lib0Any::BigInt(layout as i64)
  }
}

impl From<ViewLayout> for i64 {
  fn from(layout: ViewLayout) -> Self {
    layout as i64
  }
}
