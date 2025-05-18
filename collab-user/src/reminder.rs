use std::collections::HashMap;

use collab::preclude::encoding::serde::from_any;
use collab::preclude::{
  Any, Array, ArrayRef, Change, DeepObservable, Event, Map, MapPrelim, MapRef, Out, ReadTxn,
  Subscription, ToJson, TransactionMut, YrsValue,
};
use collab_entity::reminder::{
  REMINDER_ID, REMINDER_IS_ACK, REMINDER_IS_READ, REMINDER_MESSAGE, REMINDER_META,
  REMINDER_OBJECT_ID, REMINDER_SCHEDULED_AT, REMINDER_TITLE, REMINDER_TY, Reminder,
};
use tokio::sync::broadcast;

pub type RemindersChangeSender = broadcast::Sender<ReminderChange>;
pub type RemindersChangeReceiver = broadcast::Receiver<ReminderChange>;

#[derive(Debug, Clone)]
pub enum ReminderChange {
  DidCreateReminders { reminders: Vec<Reminder> },
  DidDeleteReminder { index: u32 },
}

pub struct Reminders {
  pub(crate) container: ArrayRef,
  #[allow(dead_code)]
  subscription: Option<Subscription>,
}

impl Reminders {
  pub fn new(mut container: ArrayRef, change_tx: Option<RemindersChangeSender>) -> Self {
    let subscription =
      change_tx.map(|change_tx| subscribe_reminder_change(&mut container, change_tx));
    Self {
      container,
      subscription,
    }
  }

  fn find<T: ReadTxn>(&self, txn: &T, reminder_id: &str) -> Option<(u32, Out)> {
    for (i, value) in self.container.iter(txn).enumerate() {
      if let Out::YMap(map) = &value {
        if let Some(Out::Any(Any::String(str))) = map.get(txn, REMINDER_ID) {
          if &*str == reminder_id {
            return Some((i as u32, value));
          }
        }
      }
    }
    None
  }

  pub fn remove(&self, txn: &mut TransactionMut, id: &str) {
    if let Some((i, _value)) = self.find(txn, id) {
      self.container.remove(txn, i);
    }
  }

  pub fn add(&self, txn: &mut TransactionMut, reminder: Reminder) {
    let map: MapPrelim = reminder.into();
    self.container.push_back(txn, map);
  }

  pub fn update_reminder<F>(&self, txn: &mut TransactionMut, reminder_id: &str, f: F)
  where
    F: FnOnce(ReminderUpdate),
  {
    if let Some((_, Out::YMap(mut map))) = self.find(txn, reminder_id) {
      let update = ReminderUpdate {
        map_ref: &mut map,
        txn,
      };
      f(update)
    }
  }

  pub fn get_all_reminders<T: ReadTxn>(&self, txn: &T) -> Vec<Reminder> {
    self
      .container
      .iter(txn)
      .flat_map(|value| {
        let json = value.to_json(txn);
        if let Ok(reminder) = from_any::<Reminder>(&json) {
          Some(reminder)
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
  root: &mut ArrayRef,
  change_tx: RemindersChangeSender,
) -> Subscription {
  root.observe_deep(move |txn, events| {
    for event in events.iter() {
      if let Event::Array(array_event) = event {
        for change in array_event.delta(txn) {
          let change_tx = change_tx.clone();
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
              let _ = change_tx.send(ReminderChange::DidCreateReminders { reminders });
            },
            Change::Removed(index) => {
              let _ = change_tx.send(ReminderChange::DidDeleteReminder { index: *index });
            },
            Change::Retain(_) => {},
          }
        }
      }
    }
  })
}

pub struct ReminderUpdate<'a, 'b> {
  map_ref: &'a mut MapRef,
  txn: &'a mut TransactionMut<'b>,
}

impl ReminderUpdate<'_, '_> {
  pub fn set_object_id<T: AsRef<str>>(self, value: T) -> Self {
    self
      .map_ref
      .try_update(self.txn, REMINDER_OBJECT_ID, value.as_ref());
    self
  }

  pub fn set_title<T: AsRef<str>>(self, value: T) -> Self {
    self
      .map_ref
      .try_update(self.txn, REMINDER_TITLE, value.as_ref());
    self
  }

  pub fn set_message<T: AsRef<str>>(self, value: T) -> Self {
    self
      .map_ref
      .try_update(self.txn, REMINDER_MESSAGE, value.as_ref());
    self
  }

  pub fn set_is_ack(self, value: bool) -> Self {
    self.map_ref.try_update(self.txn, REMINDER_IS_ACK, value);
    self
  }

  pub fn set_is_read(self, value: bool) -> Self {
    self.map_ref.try_update(self.txn, REMINDER_IS_READ, value);
    self
  }

  pub fn set_type<T: Into<i64>>(self, value: T) -> Self {
    self.map_ref.try_update(self.txn, REMINDER_TY, value.into());
    self
  }

  pub fn set_scheduled_at<T: Into<i64>>(self, value: T) -> Self {
    self
      .map_ref
      .try_update(self.txn, REMINDER_SCHEDULED_AT, value.into());
    self
  }

  pub fn set_meta(self, value: HashMap<String, String>) -> Self {
    self.map_ref.try_update(self.txn, REMINDER_META, value);
    self
  }
}
