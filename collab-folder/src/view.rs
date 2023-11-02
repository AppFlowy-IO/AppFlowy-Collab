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

use crate::folder_observe::ViewChangeSender;
use crate::section::{Section, SectionItem, SectionMap};
use crate::{
  impl_any_update, impl_i64_update, impl_option_i64_update, impl_option_str_update,
  impl_str_update, UserId,
};
use crate::{subscribe_view_change, RepeatedViewIdentifier, ViewIdentifier, ViewRelations};

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_PARENT_ID: &str = "bid";
const VIEW_DESC: &str = "desc";
const VIEW_DATABASE_ID: &str = "database_id";
const VIEW_LAYOUT: &str = "layout";
const VIEW_CREATE_AT: &str = "created_at";
const VIEW_CREATED_BY: &str = "created_by";
const VIEW_ICON: &str = "icon";
const VIEW_LAST_EDITED_TIME: &str = "last_edited_time";
const VIEW_LAST_EDITED_BY: &str = "last_edited_by";
// const VIEW_LAST_VIEWED_TIME: &str = "last_viewed_time";

pub fn timestamp() -> i64 {
  chrono::Utc::now().timestamp()
}

pub struct ViewsMap {
  uid: UserId,
  container: MapRefWrapper,
  pub(crate) view_relations: Rc<ViewRelations>,
  pub(crate) section_map: Rc<SectionMap>,
  view_cache: Arc<RwLock<HashMap<String, Arc<View>>>>,

  #[allow(dead_code)]
  subscription: Option<DeepEventsSubscription>,
  #[allow(dead_code)]
  change_tx: Option<ViewChangeSender>,
}

impl ViewsMap {
  pub fn new(
    uid: &UserId,
    mut root: MapRefWrapper,
    change_tx: Option<ViewChangeSender>,
    view_relations: Rc<ViewRelations>,
    section_map: Rc<SectionMap>,
  ) -> ViewsMap {
    let view_cache = Arc::new(RwLock::new(HashMap::new()));
    let subscription = change_tx.as_ref().map(|change_tx| {
      subscribe_view_change(
        uid,
        &mut root,
        view_cache.clone(),
        change_tx.clone(),
        view_relations.clone(),
        section_map.clone(),
      )
    });
    Self {
      uid: uid.clone(),
      container: root,
      subscription,
      change_tx,
      view_relations,
      view_cache,
      section_map,
    }
  }

  pub fn move_child(&self, parent_id: &str, from: u32, to: u32) {
    self.view_relations.move_child(parent_id, from, to);
    self.remove_cache_view(parent_id);
  }

  /// Dissociate the relationship between parent_id and view_id.
  /// Why don't we use the move method to replace dissociate_parent_child and associate_parent_child?
  /// Because the views and workspaces are stored in two separate maps, we can't directly move a view from one map to another.
  /// So, we have to dissociate the relationship between parent_id and view_id, and then associate the relationship between parent_id and view_id.
  pub fn dissociate_parent_child(&self, parent_id: &str, view_id: &str) {
    self.container.with_transact_mut(|txn| {
      self.dissociate_parent_child_with_txn(txn, parent_id, view_id);
    })
  }

  /// Establish a relationship between the parent_id and view_id, and insert the view below the prev_id.
  /// Why don't we use the move method to replace dissociate_parent_child and associate_parent_child?
  /// Because the view and workspace are stored in two separate maps, we can't directly move the view from one map to another.
  /// So we have to dissociate the relationship between parent_id and view_id, and then associate the relationship between parent_id and view_id.
  pub fn associate_parent_child(&self, parent_id: &str, view_id: &str, prev_id: Option<String>) {
    self.container.with_transact_mut(|txn| {
      self.associate_parent_child_with_txn(txn, parent_id, view_id, prev_id);
    })
  }

  pub fn dissociate_parent_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    view_id: &str,
  ) {
    self
      .view_relations
      .dissociate_parent_child_with_txn(txn, parent_id, view_id);
    self.remove_cache_view(parent_id);
  }

  pub fn associate_parent_child_with_txn(
    &self,
    txn: &mut TransactionMut,
    parent_id: &str,
    view_id: &str,
    prev_view_id: Option<String>,
  ) {
    self
      .view_relations
      .associate_parent_child_with_txn(txn, parent_id, view_id, prev_view_id);
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
                .and_then(|map| {
                  view_from_map_ref(
                    &self.uid,
                    &map,
                    txn,
                    &self.view_relations,
                    &self.section_map,
                  )
                })
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
      let view = view_from_map_ref(
        &self.uid,
        &map_ref,
        txn,
        &self.view_relations,
        &self.section_map,
      )
      .map(Arc::new);
      self.set_cache_view(view.clone());
      return view;
    }
    view
  }

  pub fn get_view_name_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<String> {
    let map_ref = self.container.get_map_with_txn(txn, view_id)?;
    map_ref.get_str_with_txn(txn, VIEW_NAME)
  }

  pub(crate) fn insert_view(&self, view: View, index: Option<u32>) {
    self
      .container
      .with_transact_mut(|txn| self.insert_view_with_txn(txn, view, index));
  }

  pub(crate) fn insert_view_with_txn(
    &self,
    txn: &mut TransactionMut,
    view: View,
    index: Option<u32>,
  ) {
    let time = timestamp();
    if let Some(parent_map_ref) = self.container.get_map_with_txn(txn, &view.parent_view_id) {
      let view_identifier = ViewIdentifier {
        id: view.id.clone(),
      };
      let view = ViewUpdate::new(
        &self.uid,
        &view.parent_view_id,
        txn,
        &parent_map_ref,
        self.view_relations.clone(),
        &self.section_map,
      )
      .add_children(vec![view_identifier], index)
      .set_created_at(time)
      .set_last_edited_time(time)
      .done()
      .map(Arc::new);
      self.set_cache_view(view);
    }

    let map_ref = self.container.create_map_with_txn(txn, &view.id);
    let view_builder = ViewBuilder::new(
      &view.id,
      txn,
      map_ref,
      self.view_relations.clone(),
      &self.section_map,
    )
    .update(&self.uid, |update| {
      let uid = self.uid.as_i64();
      let create_by = view.created_by.unwrap_or(uid);
      let last_edited_by = view.last_edited_by.unwrap_or(uid);
      let created_at = self.normalize_timestamp(view.created_at);
      let last_edited_time = self.normalize_timestamp(view.last_edited_time);
      update
        .set_name(view.name)
        .set_bid(view.parent_view_id)
        .set_desc(view.desc)
        .set_layout(view.layout)
        .set_created_at(created_at)
        .set_children(view.children)
        .set_icon(view.icon)
        .set_created_by(Some(create_by))
        .set_last_edited_time(last_edited_time)
        .set_last_edited_by(Some(last_edited_by))
        .done()
    })
    .done();
    let view = view_builder.map(Arc::new);
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
    self.remove_cache_view(view_id);
    self
      .container
      .with_transact_mut(|txn| self.update_view_with_txn(&self.uid, txn, view_id, f))
  }

  /// Updates a view within a given transaction using a provided function.
  ///
  /// This function receives a mutable reference to a transaction, `txn`, a `view_id`,
  /// and a function `f` which is applied to update the view. The function `f` takes a `ViewUpdate` as an argument
  /// and should return an updated `Option<View>`.
  ///
  /// If the specified view exists and the update function `f` returns a `Some(View)`,
  /// the function updates the cache with this new view and returns it wrapped in an `Arc<View>`.
  /// If the update function returns `None`, the function doesn't update the cache and
  /// returns `None` as well.
  ///
  /// # Type Parameters
  ///
  /// * `F` - The type of the function used to update the view. The function should accept a `ViewUpdate`
  ///   and return an `Option<View>`.
  ///
  /// # Arguments
  ///
  /// * `txn` - A mutable reference to a transaction.
  /// * `view_id` - A string slice that holds the id of the view to be updated.
  /// * `f` - A function that will be used to update the view.
  ///
  pub fn update_view_with_txn<F>(
    &self,
    uid: &UserId,
    txn: &mut TransactionMut,
    view_id: &str,
    f: F,
  ) -> Option<Arc<View>>
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    let map_ref = self.container.get_map_with_txn(txn, view_id)?;
    let update = ViewUpdate::new(
      uid,
      view_id,
      txn,
      &map_ref,
      self.view_relations.clone(),
      &self.section_map,
    )
    .set_last_edited_by(Some(uid.as_i64()))
    .set_last_edited_time(timestamp());
    let view = f(update).map(Arc::new);
    self.set_cache_view(view.clone());
    view
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

  // some history data may not have the timestamp and it's value equal to 0, so we should normalize the timestamp.
  fn normalize_timestamp(&self, timestamp: i64) -> i64 {
    if timestamp == 0 {
      chrono::Utc::now().timestamp()
    } else {
      timestamp
    }
  }
}

pub(crate) fn view_from_map_ref<T: ReadTxn>(
  _uid: &UserId,
  map_ref: &MapRef,
  txn: &T,
  view_relations: &Rc<ViewRelations>,
  section_map: &SectionMap,
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

  let icon = get_icon_from_view_map(map_ref, txn);
  let is_favorite = section_map
    .section_op_with_txn(txn, Section::Favorite)
    .map(|op| op.contains_with_txn(txn, &id))
    .unwrap_or(false);

  let created_by = map_ref.get_i64_with_txn(txn, VIEW_CREATED_BY);
  let last_edited_time = map_ref
    .get_i64_with_txn(txn, VIEW_LAST_EDITED_TIME)
    .unwrap_or_default();
  let last_edited_by = map_ref.get_i64_with_txn(txn, VIEW_LAST_EDITED_BY);

  Some(View {
    id,
    parent_view_id,
    name,
    desc,
    children,
    created_at,
    layout,
    icon,
    is_favorite,
    created_by,
    last_edited_time,
    last_edited_by,
  })
}

pub fn get_icon_from_view_map<T: ReadTxn>(map_ref: &MapRef, txn: &T) -> Option<ViewIcon> {
  let icon_str = map_ref.get_str_with_txn(txn, VIEW_ICON)?;
  serde_json::from_str::<ViewIcon>(&icon_str).ok()
}

pub struct ViewBuilder<'a, 'b> {
  view_id: &'a str,
  map_ref: MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  belongings: Rc<ViewRelations>,
  view: Option<View>,
  section_map: &'a SectionMap,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
  pub fn new(
    view_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: MapRefWrapper,
    belongings: Rc<ViewRelations>,
    section_map: &'a SectionMap,
  ) -> Self {
    map_ref.insert_str_with_txn(txn, VIEW_ID, view_id);
    Self {
      view_id,
      map_ref,
      txn,
      belongings,
      view: None,
      section_map,
    }
  }

  pub fn update<F>(mut self, uid: &UserId, f: F) -> Self
  where
    F: FnOnce(ViewUpdate) -> Option<View>,
  {
    let update = ViewUpdate::new(
      uid,
      self.view_id,
      self.txn,
      &self.map_ref,
      self.belongings.clone(),
      self.section_map,
    );
    self.view = f(update);
    self
  }
  pub fn done(self) -> Option<View> {
    self.view
  }
}

pub struct ViewUpdate<'a, 'b, 'c> {
  uid: &'a UserId,
  view_id: &'a str,
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
  children_map: Rc<ViewRelations>,
  section_map: &'c SectionMap,
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
  impl_any_update!(set_layout, set_layout_if_not_none, VIEW_LAYOUT, ViewLayout);
  impl_i64_update!(set_created_at, set_created_at_if_not_none, VIEW_CREATE_AT);
  impl_option_i64_update!(set_created_by, VIEW_CREATED_BY);
  impl_i64_update!(
    set_last_edited_time,
    set_last_edited_time_if_not_none,
    VIEW_LAST_EDITED_TIME
  );
  impl_option_i64_update!(set_last_edited_by, VIEW_LAST_EDITED_BY);

  pub fn new(
    uid: &'a UserId,
    view_id: &'a str,
    txn: &'a mut TransactionMut<'b>,
    map_ref: &'c MapRefWrapper,
    children_map: Rc<ViewRelations>,
    section_map: &'c SectionMap,
  ) -> Self {
    Self {
      uid,
      view_id,
      map_ref,
      txn,
      children_map,
      section_map,
    }
  }

  pub fn set_children(self, children: RepeatedViewIdentifier) -> Self {
    let array = self
      .children_map
      .get_or_create_children_with_txn(self.txn, self.view_id);
    array.add_children_with_txn(self.txn, children.into_inner(), None);

    self
  }

  pub fn set_icon(self, icon: Option<ViewIcon>) -> Self {
    let icon_str = icon
      .and_then(|icon| serde_json::to_string(&icon).ok())
      .unwrap_or_default();
    self
      .map_ref
      .insert_str_with_txn(self.txn, VIEW_ICON, icon_str);

    self
  }

  pub fn set_favorite(self, is_favorite: bool) -> Self {
    if let Some(fav_section) = self
      .section_map
      .section_op_with_txn(self.txn, Section::Favorite)
    {
      if is_favorite {
        fav_section
          .add_sections_item_with_txn(self.txn, vec![SectionItem::new(self.view_id.to_string())]);
      } else {
        fav_section.delete_section_items_with_txn(self.txn, vec![self.view_id.to_string()]);
      }
    }

    self
  }

  pub fn set_favorite_if_not_none(self, is_favorite: Option<bool>) -> Self {
    if let Some(is_favorite) = is_favorite {
      self.set_favorite(is_favorite)
    } else {
      self
    }
  }

  /// Add or remove the view_id from the recent section.
  ///
  /// If the view is in the recent section, it's timestamp will be updated.
  pub fn set_recent(self, add_in_recent: bool) -> Self {
    if let Some(recent_section) = self
      .section_map
      .section_op_with_txn(self.txn, Section::Recent)
    {
      // try to remove the section, if the section is not found, it will be ignored.
      recent_section.delete_section_items_with_txn(self.txn, vec![self.view_id.to_string()]);

      // add the section if add_in_recent is true since we have removed the section before.
      if add_in_recent {
        recent_section.add_sections_item_with_txn(
          self.txn,
          vec![SectionItem {
            id: self.view_id.to_string(),
            timestamp: Some(timestamp()),
          }],
        );
      }
    }

    self
  }

  pub fn add_children(self, children: Vec<ViewIdentifier>, index: Option<u32>) -> Self {
    self
      .children_map
      .add_children(self.txn, self.view_id, children, index);
    self
  }

  pub fn done(self) -> Option<View> {
    view_from_map_ref(
      self.uid,
      self.map_ref,
      self.txn,
      &self.children_map,
      self.section_map,
    )
  }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct View {
  pub id: String,
  pub parent_view_id: String,
  pub name: String,
  pub desc: String,
  pub children: RepeatedViewIdentifier,
  pub created_at: i64,
  #[serde(default)]
  pub is_favorite: bool,
  pub layout: ViewLayout,
  pub icon: Option<ViewIcon>,
  pub created_by: Option<i64>, // user id
  pub last_edited_time: i64,
  pub last_edited_by: Option<i64>, // user id
}

impl Default for View {
  fn default() -> Self {
    Self {
      id: "".to_string(),
      parent_view_id: "".to_string(),
      name: "".to_string(),
      desc: "".to_string(),
      children: Default::default(),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: None,
      last_edited_time: 0,
      last_edited_by: None,
    }
  }
}

#[derive(Eq, PartialEq, Debug, Hash, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum IconType {
  Emoji = 0,
  Url = 1,
  Icon = 2,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ViewIcon {
  pub ty: IconType,
  pub value: String,
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

  fn try_from(value: i64) -> Result<Self, Self::Error> {
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
