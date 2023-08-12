use std::collections::HashMap;

use anyhow::Result;
use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  lib0Any, Array, ArrayRefWrapper, Change, DeepEventsSubscription, DeepObservable, Event,
  MapPrelim, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub type RemindersChangeSender = broadcast::Sender<ReminderChange>;
pub type RemindersChangeReceiver = broadcast::Receiver<ReminderChange>;

#[derive(Debug, Clone)]
pub enum ReminderChange {
  DidCreateReminders { reminders: Vec<Reminder> },
  DidDeleteReminder { index: u32 },
}
pub struct Reminders {
  pub(crate) container: ArrayRefWrapper,
  #[allow(dead_code)]
  subscription: Option<DeepEventsSubscription>,
}

impl Reminders {
  pub fn new(mut container: ArrayRefWrapper, change_tx: Option<RemindersChangeSender>) -> Self {
    let subscription =
      change_tx.map(|change_tx| subscribe_reminder_change(&mut container, change_tx));
    Self {
      container,
      subscription,
    }
  }

  pub fn remove(&self, id: &str) {
    self.container.with_transact_mut(|txn| {
      self.container.remove_with_id(txn, id, REMINDER_ID);
    });
  }

  pub fn add(&self, reminder: Reminder) {
    self.container.with_transact_mut(|txn| {
      let _ = self
        .container
        .insert_map_with_txn(txn, Some(reminder.into()));
    });
  }

  pub fn update_reminder<F>(&self, reminder_id: &str, f: F)
  where
    F: FnOnce(&mut Reminder),
  {
    self.container.with_transact_mut(|txn| {
      self
        .container
        .mut_map_element_with_txn(txn, reminder_id, REMINDER_ID, |txn, map| {
          let mut reminder = Reminder::try_from((txn, map)).ok()?;
          f(&mut reminder);
          Some(MapPrelim::from(reminder))
        });
    });
  }

  pub fn get_all_reminders(&self) -> Vec<Reminder> {
    let txn = self.container.transact();
    self
      .container
      .iter(&txn)
      .flat_map(|value| {
        if let YrsValue::YMap(map) = value {
          Reminder::try_from((&txn, map)).ok()
        } else {
          None
        }
      })
      .collect()
  }
}

/// Subscribes to changes in the reminders array and dispatches relevant notifications.
///
/// The function subscribes to deep changes in the provided `ArrayRefWrapper`, filtering
/// for events specific to the reminders array. When reminders are added or removed, appropriate
/// messages are sent to the `change_tx` channel.
///
/// # Arguments
///
/// * `root` - A mutable reference to the root `ArrayRefWrapper` to observe for deep changes.
/// * `change_tx` - The sender end of a channel where changes to the reminders array will be dispatched.
///
/// # Returns
///
/// A `DeepEventsSubscription` that represents the active subscription to the array's changes.
///
fn subscribe_reminder_change(
  root: &mut ArrayRefWrapper,
  change_tx: RemindersChangeSender,
) -> DeepEventsSubscription {
  root.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      if let Event::Array(event) = deep_event {
        for change in event.delta(txn) {
          let _change_tx = change_tx.clone();
          match change {
            Change::Added(values) => {
              let reminders = values
                .iter()
                .filter_map(|value| {
                  if let YrsValue::YMap(map) = value {
                    Reminder::try_from((txn, map)).ok()
                  } else {
                    None
                  }
                })
                .collect();
              let _ = _change_tx.send(ReminderChange::DidCreateReminders { reminders });
            },
            Change::Removed(index) => {
              let _ = _change_tx.send(ReminderChange::DidDeleteReminder { index: *index });
            },
            Change::Retain(_) => {},
          }
        }
      }
    }
  })
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Reminder {
  #[serde(rename = "id")]
  pub id: String,
  pub scheduled_at: i64,
  pub is_ack: bool,
  pub ty: i64,
  pub title: String,
  pub message: String,
  pub reminder_object_id: String,
}

impl Reminder {
  pub fn new(id: String, scheduled_at: i64, ty: i64, reminder_object_id: String) -> Self {
    Self {
      id,
      scheduled_at,
      is_ack: false,
      ty,
      title: "".to_string(),
      message: "".to_string(),
      reminder_object_id,
    }
  }

  pub fn with_title(self, title: String) -> Self {
    Self { title, ..self }
  }

  pub fn with_message(self, message: String) -> Self {
    Self { message, ..self }
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

const REMINDER_ID: &str = "id";
const REMINDER_SCHEDULED_AT: &str = "scheduled_at";
const REMINDER_IS_ACK: &str = "is_ack";
const REMINDER_TY: &str = "ty";
const REMINDER_TITLE: &str = "title";
const REMINDER_MESSAGE: &str = "message";
const REMINDER_OBJECT_ID: &str = "reminder_object_id";

fn reminder_from_map<T: ReadTxn>(txn: &T, map_ref: &MapRef) -> Result<Reminder> {
  let id = map_ref
    .get_str_with_txn(txn, REMINDER_ID)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_ID))?;
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
  let reminder_object_id = map_ref
    .get_str_with_txn(txn, REMINDER_OBJECT_ID)
    .ok_or(anyhow::anyhow!("{} not found", REMINDER_OBJECT_ID))?;

  Ok(Reminder {
    id,
    scheduled_at,
    is_ack,
    ty,
    title,
    message,
    reminder_object_id,
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
      REMINDER_SCHEDULED_AT.to_string(),
      lib0Any::BigInt(item.scheduled_at),
    );
    map.insert(REMINDER_IS_ACK.to_string(), lib0Any::Bool(item.is_ack));
    map.insert(REMINDER_TY.to_string(), lib0Any::BigInt(item.ty));
    map.insert(
      REMINDER_TITLE.to_string(),
      lib0Any::String(item.title.into_boxed_str()),
    );
    map.insert(
      REMINDER_MESSAGE.to_string(),
      lib0Any::String(item.message.into_boxed_str()),
    );
    map.insert(
      REMINDER_OBJECT_ID.to_string(),
      lib0Any::String(item.reminder_object_id.into_boxed_str()),
    );
    MapPrelim::from(map)
  }
}
