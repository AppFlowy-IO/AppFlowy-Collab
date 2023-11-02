use std::collections::HashMap;

use anyhow::bail;
use collab::preclude::{
  lib0Any, Array, Map, MapRefWrapper, ReadTxn, Transact, TransactionMut, Value, YrsValue,
};
use serde::{Deserialize, Serialize};

use crate::UserId;

pub struct SectionMap {
  uid: UserId,
  container: MapRefWrapper,
}

impl SectionMap {
  pub fn create(txn: &mut TransactionMut, uid: &UserId, root: MapRefWrapper) -> Self {
    // Favorite Section
    root.create_map_with_txn_if_not_exist(txn, Section::Favorite.as_ref());
    // Recent Section
    root.create_map_with_txn_if_not_exist(txn, Section::Recent.as_ref());

    Self {
      uid: uid.clone(),
      container: root,
    }
  }
  pub fn new(uid: &UserId, root: MapRefWrapper) -> Self {
    Self {
      uid: uid.clone(),
      container: root,
    }
  }

  pub fn section_op(&self, section: Section) -> Option<SectionOperation> {
    let txn = self.container.try_transact().ok()?;
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

pub enum Section {
  Favorite,
  Recent,
  Custom(String),
}

impl From<String> for Section {
  fn from(value: String) -> Self {
    Section::Custom(value)
  }
}

impl AsRef<str> for Section {
  fn as_ref(&self) -> &str {
    match self {
      Section::Favorite => "favorite",
      Section::Recent => "recent",
      Section::Custom(s) => s.as_str(),
    }
  }
}

pub type SectionsByUid = HashMap<UserId, Vec<SectionItem>>;

pub struct SectionOperation<'a> {
  uid: &'a UserId,
  container: MapRefWrapper,
}

impl<'a> SectionOperation<'a> {
  fn container(&self) -> &MapRefWrapper {
    &self.container
  }

  fn uid(&self) -> &UserId {
    self.uid
  }

  pub fn get_sections_with_txn<T: ReadTxn>(&self, txn: &T) -> SectionsByUid {
    let mut section_id_by_uid = HashMap::new();
    for (uid, value) in self.container().iter(txn) {
      if let Value::YArray(array) = value {
        let mut items = vec![];
        for value in array.iter(txn) {
          if let YrsValue::Any(any) = value {
            items.push(SectionItem::from(any))
          }
        }

        section_id_by_uid.insert(
          UserId(uid.to_string()),
          items.into_iter().map(|item| item).collect(),
        );
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
            sections.push(SectionItem::from(any))
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
    }
  }

  #[allow(dead_code)]
  pub fn add_section_items(&self, items: Vec<SectionItem>) {
    self.container().with_transact_mut(|txn| {
      self.add_sections_item_with_txn(txn, items);
    });
  }

  pub fn add_sections_item_with_txn(&self, txn: &mut TransactionMut, items: Vec<SectionItem>) {
    self.add_sections_for_user_with_txn(txn, self.uid(), items);
  }

  pub fn add_sections_for_user_with_txn(
    &self,
    txn: &mut TransactionMut,
    uid: &UserId,
    favorite_records: Vec<SectionItem>,
  ) {
    let fav_array = self
      .container()
      .create_array_if_not_exist_with_txn::<SectionItem, _>(txn, uid, vec![]);

    for favorite_record in favorite_records {
      fav_array.push_back(txn, favorite_record);
    }
  }

  pub fn clear(&self) {
    self.container().with_transact_mut(|txn| {
      if let Some(fav_array) = self.container().get_array_ref_with_txn(txn, self.uid()) {
        let len = fav_array.iter(txn).count();
        fav_array.remove_range(txn, 0, len as u32);
      }
    });
  }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SectionItem {
  // view_id
  pub id: String,

  // the timestamp when the item was added to the section
  #[serde(skip_serializing_if = "Option::is_none")]
  pub timestamp: Option<i64>,
}

impl SectionItem {
  pub fn new(id: String) -> Self {
    Self {
      id,
      timestamp: None,
    }
  }
}

impl Eq for SectionItem {}

impl From<lib0Any> for SectionItem {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<SectionItem> for lib0Any {
  fn from(item: SectionItem) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

impl TryFrom<&YrsValue> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(value: &Value) -> Result<Self, Self::Error> {
    match value {
      Value::Any(any) => Ok(SectionItem::from(any.clone())),
      _ => bail!("Invalid section yrs value"),
    }
  }
}
