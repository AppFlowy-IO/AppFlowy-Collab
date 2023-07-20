use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::bail;
use collab::preclude::{
  lib0Any, DeepEventsSubscription, MapRef, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_repr::*;

use crate::core::folder_observe::ViewChangeSender;
use crate::core::{subscribe_view_change, RepeatedViewIdentifier, ViewIdentifier, ViewRelations};
use crate::{
  impl_any_update, impl_bool_update, impl_i64_update, impl_option_str_update, impl_str_update,
};

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_PARENT_ID: &str = "bid";
const VIEW_DESC: &str = "desc";
const VIEW_DATABASE_ID: &str = "database_id";
const VIEW_LAYOUT: &str = "layout";
const VIEW_CREATE_AT: &str = "created_at";
const VIEW_ICON_URL: &str = "icon_url";
const VIEW_COVER_URL: &str = "cover_url";
const FAVORITE_STATUS: &str = "is_favorite";

pub struct ViewsMap {
  container: MapRefWrapper,
  view_relations: Rc<ViewRelations>,
  view_cache: Arc<RwLock<HashMap<String, Arc<View>>>>,

  #[allow(dead_code)]
  subscription: Option<DeepEventsSubscription>,
  #[allow(dead_code)]
  change_tx: Option<ViewChangeSender>,
}

impl ViewsMap {
  pub fn new(
    mut root: MapRefWrapper,
    change_tx: Option<ViewChangeSender>,
    view_relations: Rc<ViewRelations>,
  ) -> ViewsMap {
    let view_cache = Arc::new(RwLock::new(HashMap::new()));
    let subscription = change_tx.as_ref().map(|change_tx| {
      subscribe_view_change(
        &mut root,
        view_cache.clone(),
        change_tx.clone(),
        view_relations.clone(),
      )
    });
    Self {
      container: root,
      subscription,
      change_tx,
      view_relations,
      view_cache,
    }
  }

  pub fn move_child(&self, parent_id: &str, from: u32, to: u32) {
    self.view_relations.move_child(parent_id, from, to);
    self.remove_cache_view(parent_id);
  }

  pub fn remove_child(&self, parent_id: &str, child_index: u32) {
    self.container.with_transact_mut(|txn| {
      if let Some(parent) = self.view_relations.get_children_with_txn(txn, parent_id) {
        if let Some(identifier) = parent.remove_child_with_txn(txn, child_index) {
          self.delete_views_with_txn(txn, vec![identifier.id])
        }
      }
    });
  }

  pub fn get_views_belong_to(&self, parent_view_id: &str) -> Vec<Arc<View>> {
    let txn = self.container.transact();
    self.get_views_belong_to_with_txn(&txn, parent_view_id)
  }

  pub fn get_views_belong_to_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    parent_view_id: &str,
  ) -> Vec<Arc<View>> {
    match self.get_view_with_txn(txn, parent_view_id) {
      Some(root_view) => root_view
        .children
        .iter()
        .flat_map(|child| {
          let cache_view = self.get_cache_view(txn, &child.id);
          match cache_view {
            None => {
              let view = self
                .container
                .get_map_with_txn(txn, &child.id)
                .and_then(|map| view_from_map_ref(&map, txn, &self.view_relations))
                .map(Arc::new);
              self.set_cache_view(view.clone());
              view
            },
            Some(view) => Some(view),
          }
        })
        .collect::<Vec<Arc<View>>>(),
      None => {
        let child_view_ids = self
          .view_relations
          .get_children(parent_view_id)
          .map(|array| {
            array
              .get_children_with_txn(txn)
              .into_inner()
              .into_iter()
              .map(|identifier| identifier.id)
              .collect::<Vec<String>>()
          })
          .unwrap_or_default();

        self.get_views(&child_view_ids)
      },
    }
  }

  pub fn get_views<T: AsRef<str>>(&self, view_ids: &[T]) -> Vec<Arc<View>> {
    let txn = self.container.transact();
    self.get_views_with_txn(&txn, view_ids)
  }

  pub fn get_views_with_txn<T: ReadTxn, V: AsRef<str>>(
    &self,
    txn: &T,
    view_ids: &[V],
  ) -> Vec<Arc<View>> {
    view_ids
      .iter()
      .flat_map(|view_id| self.get_view_with_txn(txn, view_id.as_ref()))
      .collect::<Vec<_>>()
  }

  pub fn get_view(&self, view_id: &str) -> Option<Arc<View>> {
    let txn = self.container.transact();
    self.get_view_with_txn(&txn, view_id)
  }

  /// Return the view with the given view id.
  /// The view is support nested, by default, we only load the view and its children.
  pub fn get_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<Arc<View>> {
    let view = self.get_cache_view(txn, view_id);
    if view.is_none() {
      let map_ref = self.container.get_map_with_txn(txn, view_id)?;
      let view = view_from_map_ref(&map_ref, txn, &self.view_relations).map(Arc::new);
      self.set_cache_view(view.clone());
      return view;
    }
    view
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
    if let Some(parent_map_ref) = self.container.get_map_with_txn(txn, &view.parent_view_id) {
      let view_identifier = ViewIdentifier {
        id: view.id.clone(),
      };
      let view = ViewUpdate::new(
        &view.parent_view_id,
        txn,
        &parent_map_ref,
        self.view_relations.clone(),
      )
      .add_children(vec![view_identifier])
      .done()
      .map(Arc::new);
      self.set_cache_view(view);
    }

    let map_ref = self.container.insert_map_with_txn(txn, &view.id);
    let view = ViewBuilder::new(&view.id, txn, map_ref, self.view_relations.clone())
      .update(|update| {
        update
          .set_name(view.name)
          .set_bid(view.parent_view_id)
          .set_desc(view.desc)
          .set_layout(view.layout)
          .set_created_at(view.created_at)
          .set_children(view.children)
          .set_favorite(view.is_favorite)
          .set_icon_url_if_not_none(view.icon_url)
          .set_cover_url_if_not_none(view.cover_url)
          .done()
      })
      .done()
      .map(Arc::new);
    self.set_cache_view(view);
  }

  pub fn delete_views<T: AsRef<str>>(&self, view_ids: Vec<T>) {
    self
      .container
      .with_transact_mut(|txn| self.delete_views_with_txn(txn, view_ids));
  }

  pub fn delete_views_with_txn<T: AsRef<str>>(&self, txn: &mut TransactionMut, view_ids: Vec<T>) {
    view_ids.iter().for_each(|view_id| {
      self.container.delete_with_txn(txn, view_id.as_ref());
    });
  }

  pub fn update_view<F>(&self, view_id: &str, f: F) -> Option<Arc<View>>
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    self.container.with_transact_mut(|txn| {
      let map_ref = self.container.get_map_with_txn(txn, view_id)?;
      let update = ViewUpdate::new(view_id, txn, &map_ref, self.view_relations.clone());
      let view = f(update).map(Arc::new);
      self.set_cache_view(view.clone());
      view
    })
  }

  fn set_cache_view(&self, view: Option<Arc<View>>) {
    if let Some(view) = view {
      self.view_cache.write().insert(view.id.clone(), view);
    }
  }

  fn get_cache_view<T: ReadTxn>(&self, _txn: &T, view_id: &str) -> Option<Arc<View>> {
    self.view_cache.read().get(view_id).cloned()
  }

  fn remove_cache_view(&self, view_id: &str) {
    self.view_cache.write().remove(view_id);
  }
}

pub(crate) fn view_from_map_ref<T: ReadTxn>(
  map_ref: &MapRef,
  txn: &T,
  view_relations: &Rc<ViewRelations>,
) -> Option<View> {
  let parent_view_id = map_ref.get_str_with_txn(txn, VIEW_PARENT_ID)?;
  let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
  let name = map_ref.get_str_with_txn(txn, VIEW_NAME).unwrap_or_default();
  let desc = map_ref.get_str_with_txn(txn, VIEW_DESC).unwrap_or_default();
  let created_at = map_ref
    .get_i64_with_txn(txn, VIEW_CREATE_AT)
    .unwrap_or_default();
  let layout = map_ref
    .get_i64_with_txn(txn, VIEW_LAYOUT)
    .map(|value| value.try_into().ok())??;

  let children = view_relations
    .get_children_with_txn(txn, &id)
    .map(|array| array.get_children_with_txn(txn))
    .unwrap_or_default();

  let is_favorite = map_ref
    .get_bool_with_txn(txn, FAVORITE_STATUS)
    .unwrap_or_default();
  let icon_url = map_ref.get_str_with_txn(txn, VIEW_ICON_URL);
  let cover_url = map_ref.get_str_with_txn(txn, VIEW_COVER_URL);

  Some(View {
    id,
    parent_view_id,
    name,
    desc,
    children,
    created_at,
    layout,
    icon_url,
    cover_url,
    is_favorite,
  })
}

pub struct ViewBuilder<'a, 'b> {
  view_id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  belongings: Rc<ViewRelations>,
  view: Option<View>,
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
      view: None,
    }
  }

  pub fn update<F>(mut self, f: F) -> Self
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    let update = ViewUpdate::new(
      self.view_id,
      self.txn,
      &self.map_ref,
      self.belongings.clone(),
    );
    self.view = f(update);
    self
  }
  pub fn done(self) -> Option<View> {
    self.view
  }
}

pub struct ViewUpdate<'a, 'b, 'c> {
  view_id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  children_map: Rc<ViewRelations>,
}

impl<'a, 'b, 'c> ViewUpdate<'a, 'b, 'c> {
  impl_str_update!(set_name, set_name_if_not_none, VIEW_NAME);
  impl_str_update!(set_bid, set_bid_if_not_none, VIEW_PARENT_ID);
  impl_option_str_update!(
    set_database_id,
    set_database_id_if_not_none,
    VIEW_DATABASE_ID
  );
  impl_str_update!(set_desc, set_desc_if_not_none, VIEW_DESC);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, VIEW_CREATE_AT);
  impl_any_update!(set_layout, set_layout_if_not_none, VIEW_LAYOUT, ViewLayout);
  impl_str_update!(icon_url, set_icon_url_if_not_none, VIEW_ICON_URL);
  impl_str_update!(cover_url, set_cover_url_if_not_none, VIEW_COVER_URL);
  impl_bool_update!(set_favorite, set_favorite_if_not_none, FAVORITE_STATUS);
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

  pub fn set_children(self, children: RepeatedViewIdentifier) -> Self {
    let array = self
      .children_map
      .get_or_create_children_with_txn(self.txn, self.view_id);
    array.add_children_with_txn(self.txn, children.into_inner());

    self
  }

  pub fn add_children(self, children: Vec<ViewIdentifier>) -> Self {
    self
      .children_map
      .add_children(self.txn, self.view_id, children);
    self
  }

  pub fn done(self) -> Option<View> {
    view_from_map_ref(self.map_ref, self.txn, &self.children_map)
  }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct View {
  pub id: String,
  pub parent_view_id: String,
  pub name: String,
  pub desc: String,
  pub children: RepeatedViewIdentifier,
  pub created_at: i64,
  pub is_favorite: bool,
  pub layout: ViewLayout,
  pub icon_url: Option<String>,
  pub cover_url: Option<String>,
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
