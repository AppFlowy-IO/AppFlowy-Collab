use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use anyhow::Result;
use collab::preclude::{lib0Any, MapPrelim, MapRef, MapRefExtension, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Reminder {
  #[serde(rename = "id")]
  pub id: String,
  pub scheduled_at: i64,
  pub is_ack: bool,
  pub ty: ObjectType,
  pub title: String,
  pub message: String,
  /// The meta field is used to store arbitrary key-value pairs.
  pub meta: ReminderMeta,
  /// The object_id field is used to store the id of the object that the reminder is associated with.
  pub object_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(i64)]
pub enum ObjectType {
  Unknown = 0,
  Document = 1,
  Database = 2,
}

impl From<i64> for ObjectType {
  fn from(value: i64) -> Self {
    match value {
      1 => ObjectType::Document,
      2 => ObjectType::Database,
      _ => ObjectType::Unknown,
    }
  }
}

impl Reminder {
  pub fn new(id: String, object_id: String, scheduled_at: i64, ty: ObjectType) -> Self {
    Self {
      id,
      scheduled_at,
      is_ack: false,
      ty,
      title: "".to_string(),
      message: "".to_string(),
      meta: ReminderMeta::default(),
      object_id,
    }
  }

  pub fn with_title(self, title: String) -> Self {
    Self { title, ..self }
  }

  pub fn with_message(self, message: String) -> Self {
    Self { message, ..self }
  }

  pub fn with_key_value<K: AsRef<str>, V: ToString>(mut self, key: K, value: V) -> Self {
    self
      .meta
      .insert(key.as_ref().to_string(), value.to_string());
    self
  }
}

impl<T> TryFrom<(&T, MapRef)> for Reminder
where
  T: ReadTxn,
{
  type Error = anyhow::Error;

  fn try_from(value: (&T, MapRef)) -> Result<Self, Self::Error> {
    let (txn, map_ref) = value;
    reminder_from_map(txn, &map_ref)
  }
}

impl<'a> TryFrom<(&mut TransactionMut<'a>, &MapRef)> for Reminder {
  type Error = anyhow::Error;

  fn try_from(value: (&mut TransactionMut, &MapRef)) -> Result<Self, Self::Error> {
    let (txn, map_ref) = value;
    reminder_from_map(txn, map_ref)
  }
}

impl<'a> TryFrom<(&TransactionMut<'a>, &MapRef)> for Reminder {
  type Error = anyhow::Error;

  fn try_from(value: (&TransactionMut, &MapRef)) -> Result<Self, Self::Error> {
    let (txn, map_ref) = value;
    reminder_from_map(txn, map_ref)
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReminderMeta(HashMap<String, String>);

impl ReminderMeta {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, String> {
    self.0
  }
}
impl Deref for ReminderMeta {
  type Target = HashMap<String, String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for ReminderMeta {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl From<HashMap<String, String>> for ReminderMeta {
  fn from(value: HashMap<String, String>) -> Self {
    Self(value)
  }
}

impl From<ReminderMeta> for lib0Any {
  fn from(value: ReminderMeta) -> Self {
    let map = value.0.into_iter().map(|(k, v)| (k, v.into())).collect();
    lib0Any::Map(Box::new(map))
  }
}

impl From<lib0Any> for ReminderMeta {
  fn from(value: lib0Any) -> Self {
    match value {
      lib0Any::Map(map) => ReminderMeta(
        map
          .into_iter()
          .map(|(k, v)| (k, v.to_string()))
          .collect::<HashMap<String, String>>(),
      ),
      _ => Default::default(),
    }
  }
}

pub const REMINDER_ID: &str = "id";
pub const REMINDER_OBJECT_ID: &str = "object_id";
pub const REMINDER_SCHEDULED_AT: &str = "scheduled_at";
pub const REMINDER_IS_ACK: &str = "is_ack";
pub const REMINDER_TY: &str = "ty";
pub const REMINDER_TITLE: &str = "title";
pub const REMINDER_MESSAGE: &str = "message";
pub const REMINDER_META: &str = "meta";

fn reminder_from_map<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Result<Reminder> {
  let id = map_ref
    .get_str_with_txn(txn, REMINDER_ID)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_ID))?;
  let object_id = map_ref
      .get_str_with_txn(txn, REMINDER_OBJECT_ID)
      .ok_or(anyhow::anyhow!("{} not found", REMINDER_OBJECT_ID))?;
  let scheduled_at = map_ref
    .get_i64_with_txn(txn, REMINDER_SCHEDULED_AT)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_SCHEDULED_AT))?;
  let is_ack = map_ref
    .get_bool_with_txn(txn, REMINDER_IS_ACK)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_IS_ACK))?;
  let ty = map_ref
    .get_i64_with_txn(txn, REMINDER_TY)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_TY))?;
  let title = map_ref
    .get_str_with_txn(txn, REMINDER_TITLE)
    .unwrap_or_default();
  let message = map_ref
    .get_str_with_txn(txn, REMINDER_MESSAGE)
    .unwrap_or_default();

  let meta = map_ref
    .get_any_with_txn(txn, REMINDER_META)
    .map(ReminderMeta::from)
    .unwrap_or_default();

  Ok(Reminder {
    id,
    object_id,
    scheduled_at,
    is_ack,
    ty: ObjectType::from(ty),
    title,
    message,
    meta,
  })
}

impl From<Reminder> for MapPrelim<lib0Any> {
  fn from(item: Reminder) -> Self {
    let mut map = HashMap::new();
    map.insert(
      REMINDER_ID.to_string(),
      lib0Any::String(item.id.into_boxed_str()),
    );
    map.insert(
      REMINDER_OBJECT_ID.to_string(),
      lib0Any::String(item.object_id.into_boxed_str()),
    );
    map.insert(
      REMINDER_SCHEDULED_AT.to_string(),
      lib0Any::BigInt(item.scheduled_at),
    );
    map.insert(REMINDER_IS_ACK.to_string(), lib0Any::Bool(item.is_ack));
    map.insert(REMINDER_TY.to_string(), lib0Any::BigInt(item.ty as i64));
    map.insert(
      REMINDER_TITLE.to_string(),
      lib0Any::String(item.title.into_boxed_str()),
    );
    map.insert(
      REMINDER_MESSAGE.to_string(),
      lib0Any::String(item.message.into_boxed_str()),
    );

    map.insert(REMINDER_META.to_string(), item.meta.into());

    MapPrelim::from(map)
  }
}
