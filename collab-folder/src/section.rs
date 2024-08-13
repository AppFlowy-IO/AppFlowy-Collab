use std::collections::HashMap;

use crate::{timestamp, UserId};
use anyhow::bail;
use collab::core::any_map::{AnyMap, AnyMapExtension};
use collab::preclude::{
  Any, Array, DeepObservable, Map, MapRefWrapper, ReadTxn, Subscription, TransactionMut, YrsValue,
};
use collab::util::deserialize_i64_from_numeric;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::info;

pub struct SectionMap {
  uid: UserId,
  container: MapRefWrapper,
  #[allow(dead_code)]
  change_tx: Option<SectionChangeSender>,
  #[allow(dead_code)]
  subscription: Subscription,
}

impl SectionMap {
  /// Creates a new section map and initializes it with default sections.
  ///
  /// This function will iterate over a predefined list of sections and
  /// create them in the provided `MapRefWrapper` if they do not exist.
  pub fn create(
    txn: &mut TransactionMut,
    uid: &UserId,
    mut root: MapRefWrapper,
    change_tx: Option<SectionChangeSender>,
  ) -> Self {
    for section in predefined_sections() {
      root.create_map_with_txn_if_not_exist(txn, section.as_ref());
    }

    let subscription = subscribe_section_change(&mut root);
    Self {
      uid: uid.clone(),
      container: root,
      change_tx,
      subscription,
    }
  }

  /// Attempts to create a new `SectionMap` from the given `MapRefWrapper`.
  ///
  /// Iterates over a list of predefined sections. If any section does not exist in the `MapRefWrapper`,
  /// logs an informational message and returns `None`. Otherwise, returns `Some(SectionMap)`.
  ///
  /// When returning None, the caller should call the [Self::create] method to create the section.
  pub fn new<T: ReadTxn>(
    txn: &T,
    uid: &UserId,
    mut root: MapRefWrapper,
    change_tx: Option<SectionChangeSender>,
  ) -> Option<Self> {
    for section in predefined_sections() {
      if root.get_map_with_txn(txn, section.as_ref()).is_none() {
        info!(
          "Section {} not exist for user {}",
          section.as_ref(),
          uid.as_ref()
        );
        return None;
      }
    }

    let subscription = subscribe_section_change(&mut root);
    Some(Self {
      uid: uid.clone(),
      container: root,
      change_tx,
      subscription,
    })
  }

  pub fn section_op(&self, section: Section) -> Option<SectionOperation> {
    let txn = self.container.collab_ctx.transact();
    self.section_op_with_txn(&txn, section)
  }

  pub fn section_op_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    section: Section,
  ) -> Option<SectionOperation> {
    let container = self.get_section(txn, section.as_ref())?;
    Some(SectionOperation {
      uid: &self.uid,
      container,
      section,
      change_tx: self.change_tx.clone(),
    })
  }

  pub fn create_section_with_txn(
    &self,
    txn: &mut TransactionMut,
    section: Section,
  ) -> MapRefWrapper {
    self
      .container
      .create_map_with_txn_if_not_exist(txn, section.as_ref())
  }

  fn get_section<T: ReadTxn>(&self, txn: &T, section_id: &str) -> Option<MapRefWrapper> {
    self.container.get_map_with_txn(txn, section_id)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section {
  Favorite,
  Recent,
  Trash,
  Private,
  Custom(String),
}

pub(crate) fn predefined_sections() -> Vec<Section> {
  vec![
    Section::Favorite,
    Section::Recent,
    Section::Trash,
    Section::Private,
  ]
}

impl From<String> for Section {
  fn from(value: String) -> Self {
    Section::Custom(value)
  }
}

impl AsRef<str> for Section {
  fn as_ref(&self) -> &str {
    // Must be unique
    match self {
      Section::Favorite => "favorite",
      Section::Recent => "recent",
      Section::Trash => "trash",
      Section::Private => "private",
      Section::Custom(s) => s.as_str(),
    }
  }
}

#[derive(Clone, Debug)]
pub enum SectionChange {
  Trash(TrashSectionChange),
}

pub type SectionChangeSender = broadcast::Sender<SectionChange>;
pub type SectionChangeReceiver = broadcast::Receiver<SectionChange>;

#[derive(Clone, Debug)]
pub enum TrashSectionChange {
  TrashItemAdded { ids: Vec<String> },
  TrashItemRemoved { ids: Vec<String> },
}

pub type SectionsByUid = HashMap<UserId, Vec<SectionItem>>;

pub struct SectionOperation<'a> {
  uid: &'a UserId,
  container: MapRefWrapper,
  section: Section,
  change_tx: Option<SectionChangeSender>,
}

impl<'a> SectionOperation<'a> {
  fn container(&self) -> &MapRefWrapper {
    &self.container
  }

  fn uid(&self) -> &UserId {
    self.uid
  }

  #[allow(dead_code)]
  pub fn get_sections(&self) -> SectionsByUid {
    let txn = self.container().transact();
    self.get_sections_with_txn(&txn)
  }

  pub fn get_sections_with_txn<T: ReadTxn>(&self, txn: &T) -> SectionsByUid {
    let mut section_id_by_uid = HashMap::new();
    for (uid, value) in self.container().iter(txn) {
      if let YrsValue::YArray(array) = value {
        let mut items = vec![];
        for value in array.iter(txn) {
          if let YrsValue::Any(any) = value {
            if let Ok(item) = SectionItem::try_from(&any) {
              items.push(item)
            }
          }
        }

        section_id_by_uid.insert(UserId(uid.to_string()), items);
      }
    }
    section_id_by_uid
  }

  #[allow(dead_code)]
  pub fn contains_view_id(&self, view_id: &str) -> bool {
    let txn = self.container().transact();
    self.contains_with_txn(&txn, view_id)
  }

  pub fn contains_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> bool {
    match self.container().get_array_ref_with_txn(txn, self.uid()) {
      None => false,
      Some(array) => {
        for value in array.iter(txn) {
          if let Ok(section_id) = SectionItem::try_from(&value) {
            if section_id.id == view_id {
              return true;
            }
          }
        }
        false
      },
    }
  }

  #[allow(dead_code)]
  pub fn get_all_section_item(&self) -> Vec<SectionItem> {
    let txn = self.container().transact();
    self.get_all_section_item_with_txn(&txn)
  }

  pub fn get_all_section_item_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<SectionItem> {
    match self.container().get_array_ref_with_txn(txn, self.uid()) {
      None => vec![],
      Some(array) => {
        let mut sections = vec![];
        for value in array.iter(txn) {
          if let YrsValue::Any(any) = value {
            // let start = std::time::Instant::now();
            // trace!("get_all_section_item data: {:?}", any);
            if let Ok(item) = SectionItem::try_from(&any) {
              // trace!("get_all_section_item: {:?}: {:?}", item, start.elapsed());
              sections.push(item)
            }
          }
        }
        sections
      },
    }
  }

  #[allow(dead_code)]
  pub fn delete_section_items<T: AsRef<str>>(&self, ids: Vec<T>) {
    self.container().with_transact_mut(|txn| {
      self.delete_section_items_with_txn(txn, ids);
    });
  }

  pub fn delete_section_items_with_txn<T: AsRef<str>>(
    &self,
    txn: &mut TransactionMut,
    ids: Vec<T>,
  ) {
    if let Some(fav_array) = self.container().get_array_ref_with_txn(txn, self.uid()) {
      for id in &ids {
        if let Some(pos) = self
          .get_all_section_item_with_txn(txn)
          .into_iter()
          .position(|item| item.id == id.as_ref())
        {
          fav_array.remove_with_txn(txn, pos as u32);
        }
      }

      if let Some(change_tx) = self.change_tx.as_ref() {
        match self.section {
          Section::Favorite => {},
          Section::Recent => {},
          Section::Trash => {
            let _ = change_tx.send(SectionChange::Trash(TrashSectionChange::TrashItemRemoved {
              ids: ids.into_iter().map(|id| id.as_ref().to_string()).collect(),
            }));
          },
          Section::Custom(_) => {},
          Section::Private => {},
        }
      }
    }
  }

  #[allow(dead_code)]
  pub fn add_section_items(&self, items: Vec<SectionItem>) {
    self.container().with_transact_mut(|txn| {
      self.add_sections_item_with_txn(txn, items);
    });
  }

  pub fn add_sections_item_with_txn(&self, txn: &mut TransactionMut, items: Vec<SectionItem>) {
    let item_ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    self.add_sections_for_user_with_txn(txn, self.uid(), items);
    if let Some(change_tx) = self.change_tx.as_ref() {
      match self.section {
        Section::Favorite => {},
        Section::Recent => {},
        Section::Trash => {
          let _ = change_tx.send(SectionChange::Trash(TrashSectionChange::TrashItemAdded {
            ids: item_ids,
          }));
        },
        Section::Custom(_) => {},
        Section::Private => {},
      }
    }
  }

  pub fn add_sections_for_user_with_txn(
    &self,
    txn: &mut TransactionMut,
    uid: &UserId,
    items: Vec<SectionItem>,
  ) {
    let array = self
      .container()
      .create_array_if_not_exist_with_txn::<SectionItem, _>(txn, uid, vec![]);

    for item in items {
      array.push_back(txn, item);
    }
  }

  pub fn clear(&self) {
    self.container().with_transact_mut(|txn| {
      if let Some(array) = self.container().get_array_ref_with_txn(txn, self.uid()) {
        let len = array.iter(txn).count();
        array.remove_range(txn, 0, len as u32);
      }
    });
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SectionItem {
  pub id: String,
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  pub timestamp: i64,
}

impl SectionItem {
  pub fn new(id: String) -> Self {
    Self {
      id,
      timestamp: timestamp(),
    }
  }
}

/// Uses [AnyMap] to store key-value pairs of section items, making it easy to extend in the future.
impl TryFrom<AnyMap> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(value: AnyMap) -> Result<Self, Self::Error> {
    let id = value
      .get_str_value("id")
      .ok_or_else(|| anyhow::anyhow!("missing section item id"))?;
    let timestamp = value.get_i64_value("timestamp").unwrap_or_else(timestamp);
    Ok(Self { id, timestamp })
  }
}

impl From<SectionItem> for AnyMap {
  fn from(item: SectionItem) -> Self {
    let mut map = AnyMap::new();
    map.insert_str_value("id", item.id);
    map.insert_i64_value("timestamp", item.timestamp);
    map
  }
}

impl TryFrom<&Any> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(any: &Any) -> Result<Self, Self::Error> {
    let any_map = AnyMap::from(any);
    Self::try_from(any_map)
  }
}

impl From<SectionItem> for Any {
  fn from(value: SectionItem) -> Self {
    let any_map = AnyMap::from(value);
    any_map.into()
  }
}

impl TryFrom<&YrsValue> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(value: &YrsValue) -> Result<Self, Self::Error> {
    match value {
      YrsValue::Any(any) => SectionItem::try_from(any),
      _ => bail!("Invalid section yrs value"),
    }
  }
}

fn subscribe_section_change(map: &mut MapRefWrapper) -> Subscription {
  map.observe_deep(move |_txn, _events| {})
}
