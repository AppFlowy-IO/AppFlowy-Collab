use std::collections::HashMap;
use std::sync::Arc;

use super::{UserId, ViewId, timestamp};
use crate::preclude::encoding::serde::{from_any, to_any};
use crate::preclude::{
  Any, AnyMut, Array, Map, MapRef, ReadTxn, Subscription, TransactionMut, YrsValue,
};
use crate::preclude::{ArrayRef, MapExt, deserialize_i64_from_numeric};
use anyhow::bail;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

pub struct SectionMap {
  container: MapRef,
  #[allow(dead_code)]
  change_tx: Option<SectionChangeSender>,
  #[allow(dead_code)]
  subscription: Option<Subscription>,
}

impl SectionMap {
  /// Creates a new section map and initializes it with default sections.
  ///
  /// This function will iterate over a predefined list of sections and
  /// create them in the provided `MapRefWrapper` if they do not exist.
  pub fn create(
    txn: &mut TransactionMut,
    root: MapRef,
    change_tx: Option<SectionChangeSender>,
  ) -> Self {
    for section in predefined_sections() {
      root.get_or_init_map(txn, section.as_ref());
    }

    Self {
      container: root,
      change_tx,
      subscription: None,
    }
  }

  pub fn section_op<T: ReadTxn>(
    &self,
    txn: &T,
    section: Section,
    uid: Option<i64>,
  ) -> Option<SectionOperation> {
    let container = self.get_section(txn, section.as_ref())?;
    Some(SectionOperation {
      uid: uid.map(UserId::from),
      container,
      section,
      change_tx: self.change_tx.clone(),
    })
  }

  pub fn create_section(&self, txn: &mut TransactionMut, section: Section) -> MapRef {
    self.container.get_or_init_map(txn, section.as_ref())
  }

  fn get_section<T: ReadTxn>(&self, txn: &T, section_id: &str) -> Option<MapRef> {
    self.container.get_with_txn(txn, section_id)
  }
}

/// Represents different types of user-specific view collections in a folder.
///
/// Sections are **per-user** organizational categories that allow each user in a
/// collaborative folder to maintain their own personal view collections. Each section
/// type has a specific semantic purpose.
///
/// # Section Types
///
/// ## Predefined Sections
///
/// - **Favorite**: Views the user has marked as favorites for quick access
/// - **Recent**: Recently accessed views, typically ordered by access time
/// - **Trash**: Views the user has deleted (pending permanent removal)
/// - **Private**: Views that are private to the user and hidden from others
///
/// ## Custom Sections
///
/// - **Custom(String)**: User-defined section types for extensibility
///
/// # Storage Architecture
///
/// Each section in the CRDT is stored as a nested map structure:
///
/// ```text
/// SectionMap
///   ├─ "favorite" (MapRef)
///   │   ├─ "1" (uid) → Array[SectionItem, SectionItem, ...]
///   │   ├─ "2" (uid) → Array[SectionItem, SectionItem, ...]
///   │   └─ ...
///   ├─ "recent" (MapRef)
///   │   └─ ...
///   ├─ "trash" (MapRef)
///   │   └─ ...
///   └─ "private" (MapRef)
///       └─ ...
/// ```
///
/// This allows multiple users to collaborate on the same folder while maintaining
/// independent personal collections. For example:
/// - User 1's favorites don't affect User 2's favorites
/// - Each user has their own trash bin
/// - Each user has their own private views
///
/// # String Representation
///
/// Each section variant has a unique string identifier used as the CRDT map key:
/// - `Favorite` → `"favorite"`
/// - `Recent` → `"recent"`
/// - `Trash` → `"trash"`
/// - `Private` → `"private"`
/// - `Custom("my_section")` → `"my_section"`
///
/// # Examples
///
/// ```rust,no_run
/// use collab::folder::Section;
///
/// // Predefined sections
/// let fav = Section::Favorite;
/// assert_eq!(fav.as_ref(), "favorite");
///
/// // Custom section
/// let custom = Section::from("my_custom_section".to_string());
/// assert_eq!(custom.as_ref(), "my_custom_section");
/// ```
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
  TrashItemAdded { ids: Vec<ViewId> },
  TrashItemRemoved { ids: Vec<ViewId> },
}

pub type SectionsByUid = HashMap<UserId, Vec<SectionItem>>;

pub struct SectionOperation {
  uid: Option<UserId>,
  container: MapRef,
  section: Section,
  change_tx: Option<SectionChangeSender>,
}

impl SectionOperation {
  fn container(&self) -> &MapRef {
    &self.container
  }

  fn uid(&self) -> Option<&UserId> {
    self.uid.as_ref()
  }

  pub fn get_sections<T: ReadTxn>(&self, txn: &T) -> SectionsByUid {
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

  pub fn contains_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &ViewId) -> bool {
    let Some(uid) = self.uid() else {
      return false;
    };
    match self
      .container()
      .get_with_txn::<_, ArrayRef>(txn, uid.as_ref())
    {
      None => false,
      Some(array) => {
        for value in array.iter(txn) {
          if let Ok(section_id) = SectionItem::try_from(&value) {
            if &section_id.id == view_id {
              return true;
            }
          }
        }
        false
      },
    }
  }

  pub fn get_all_section_item<T: ReadTxn>(&self, txn: &T) -> Vec<SectionItem> {
    let Some(uid) = self.uid() else {
      return vec![];
    };
    match self
      .container()
      .get_with_txn::<_, ArrayRef>(txn, uid.as_ref())
    {
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

  pub fn move_section_item_with_txn<T: AsRef<str>>(
    &self,
    txn: &mut TransactionMut,
    id: T,
    prev_id: Option<T>,
  ) {
    let Some(uid) = self.uid() else {
      return;
    };
    let section_items = self.get_all_section_item(txn);
    let id = id.as_ref();
    let old_pos = section_items
      .iter()
      .position(|item| item.id.to_string() == id)
      .map(|pos| pos as u32);
    let new_pos = prev_id
      .and_then(|prev_id| {
        section_items
          .iter()
          .position(|item| item.id.to_string() == prev_id.as_ref())
          .map(|pos| pos as u32 + 1)
      })
      .unwrap_or(0);
    let section_array = self
      .container()
      .get_with_txn::<_, ArrayRef>(txn, uid.as_ref());
    // If the new position index is greater than the length of the section, yrs will panic
    if new_pos > section_items.len() as u32 {
      return;
    }

    if let (Some(old_pos), Some(section_array)) = (old_pos, section_array) {
      section_array.move_to(txn, old_pos, new_pos);
    }
  }

  pub fn delete_section_items_with_txn<T: AsRef<str>>(
    &self,
    txn: &mut TransactionMut,
    ids: Vec<T>,
  ) {
    let Some(uid) = self.uid() else {
      return;
    };
    if let Some(fav_array) = self
      .container()
      .get_with_txn::<_, ArrayRef>(txn, uid.as_ref())
    {
      for id in &ids {
        if let Some(pos) = self
          .get_all_section_item(txn)
          .into_iter()
          .position(|item| item.id.to_string() == id.as_ref())
        {
          fav_array.remove(txn, pos as u32);
        }
      }

      if let Some(change_tx) = self.change_tx.as_ref() {
        match self.section {
          Section::Favorite => {},
          Section::Recent => {},
          Section::Trash => {
            let _ = change_tx.send(SectionChange::Trash(TrashSectionChange::TrashItemRemoved {
              ids: ids
                .into_iter()
                .filter_map(|id| Uuid::parse_str(id.as_ref()).ok())
                .collect(),
            }));
          },
          Section::Custom(_) => {},
          Section::Private => {},
        }
      }
    }
  }

  pub fn add_sections_item(&self, txn: &mut TransactionMut, items: Vec<SectionItem>) {
    let Some(uid) = self.uid() else {
      return;
    };
    let item_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    self.add_sections_for_user_with_txn(txn, uid, items);
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
    let array = self.container().get_or_init_array(txn, uid.as_ref());

    for item in items {
      array.push_back(txn, item);
    }
  }

  pub fn clear(&self, txn: &mut TransactionMut) {
    let Some(uid) = self.uid() else {
      return;
    };
    if let Some(array) = self
      .container()
      .get_with_txn::<_, ArrayRef>(txn, uid.as_ref())
    {
      let len = array.iter(txn).count();
      array.remove_range(txn, 0, len as u32);
    }
  }
}

/// An item in a user's section, representing a view and when it was added.
///
/// `SectionItem` is the fundamental unit stored in sections (Favorite, Recent, Trash, Private).
/// Each item records both which view is in the section and when it was added, enabling
/// time-based operations like sorting by recency or tracking deletion times.
///
/// # Fields
///
/// * `id` - The UUID of the view in this section
/// * `timestamp` - Unix timestamp (milliseconds) when the view was added to this section
///
/// # Storage Format
///
/// SectionItems are serialized as Yrs `Any` values (essentially JSON-like maps) in the CRDT:
///
/// ```json
/// {
///   "id": "550e8400-e29b-41d4-a716-446655440000",
///   "timestamp": 1704067200000
/// }
/// ```
///
/// Multiple items are stored in a Yrs array per user per section:
///
/// ```text
/// section["favorite"]["123"] = [
///   SectionItem { id: view_1, timestamp: 1704067200000 },
///   SectionItem { id: view_2, timestamp: 1704068000000 },
///   ...
/// ]
/// ```
///
/// # Usage Patterns
///
/// ## Favorite Sections
/// ```rust,no_run
/// # use collab::folder::Folder;
/// # let folder: Folder = unimplemented!();
/// # let uid = 1_i64;
/// let favorites = folder.get_my_favorite_sections(Some(uid));
/// for item in favorites {
///     println!("View {} favorited at {}", item.id, item.timestamp);
/// }
/// ```
///
/// ## Recent Sections (sorted by timestamp)
/// ```rust,no_run
/// # use collab::folder::SectionItem;
/// let mut recent = vec![
///     SectionItem {
///         id: uuid::Uuid::nil(),
///         timestamp: 1704067200000,
///     },
///     SectionItem {
///         id: uuid::Uuid::from_u128(1),
///         timestamp: 1704068200000,
///     },
/// ];
/// recent.sort_by_key(|item| std::cmp::Reverse(item.timestamp)); // newest first
/// let most_recent = recent.first();
/// ```
///
/// ## Trash Sections (check deletion time)
/// ```rust,no_run
/// # use collab::folder::Folder;
/// # fn current_timestamp() -> i64 {
/// #     std::time::SystemTime::now()
/// #         .duration_since(std::time::UNIX_EPOCH)
/// #         .unwrap()
/// #         .as_millis() as i64
/// # }
/// # let folder: Folder = unimplemented!();
/// # let uid = 1_i64;
/// let trash = folder.get_my_trash_sections(Some(uid));
/// let thirty_days_ago = current_timestamp() - (30 * 24 * 60 * 60 * 1000);
/// let permanently_delete = trash
///     .iter()
///     .filter(|item| item.timestamp < thirty_days_ago)
///     .collect::<Vec<_>>();
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SectionItem {
  pub id: ViewId,
  #[serde(deserialize_with = "deserialize_i64_from_numeric")]
  pub timestamp: i64,
}

impl SectionItem {
  pub fn new(id: ViewId) -> Self {
    Self {
      id,
      timestamp: timestamp(),
    }
  }
}

/// Uses [AnyMap] to store key-value pairs of section items, making it easy to extend in the future.
impl TryFrom<Any> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(value: Any) -> Result<Self, Self::Error> {
    let value = from_any(&value)?;
    Ok(value)
  }
}

impl From<SectionItem> for HashMap<String, AnyMut> {
  fn from(item: SectionItem) -> Self {
    HashMap::from([
      (
        "id".to_string(),
        AnyMut::String(Arc::from(item.id.to_string())),
      ),
      (
        "timestamp".to_string(),
        AnyMut::Number(item.timestamp as f64),
      ),
    ])
  }
}

impl TryFrom<&Any> for SectionItem {
  type Error = anyhow::Error;

  fn try_from(any: &Any) -> Result<Self, Self::Error> {
    Ok(from_any(any)?)
  }
}

impl From<SectionItem> for Any {
  fn from(value: SectionItem) -> Self {
    to_any(&value).unwrap()
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
