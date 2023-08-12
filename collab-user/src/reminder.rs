use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  Array, ArrayRefWrapper, Change, DeepEventsSubscription, DeepObservable, Event, lib0Any,
  MapRefWrapper, TransactionMut, YrsValue,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub type RemindersChangeSender = broadcast::Sender<ReminderChange>;
pub type RemindersChangeReceiver = broadcast::Receiver<ReminderChange>;

#[derive(Debug, Clone)]
pub enum ReminderChange {
  DidCreateReminders { reminders: Vec<Reminder> },
  DidDeleteReminders { reminders: Vec<String> },
  DidUpdateReminders { reminders: Vec<Reminder> },
}
pub struct Reminders {
  pub(crate) container: ArrayRefWrapper,
}

impl Reminders {
  pub fn new(mut container: ArrayRefWrapper, change_tx: Option<RemindersChangeSender>) -> Self {
    if let Some(change_tx) = change_tx {
      subscribe_reminder_change(&mut container, change_tx);
    }
    Self { container }
  }

  pub fn remove(&self, id: &str) {
    self.container.with_transact_mut(|txn| {
      self.container.remove_with_id(txn, id, REMINDER_ID);
    });
  }

  pub fn add(&self, reminder: Reminder) {
    self.container.push(reminder);
  }

  pub fn update_reminder<F>(&self, reminder_id: &str, f: F)
  where
    F: FnOnce(&mut Reminder),
  {
    self.container.with_transact_mut(|txn| {
      self
        .container
        .mut_with_txn(txn, reminder_id, REMINDER_ID, |mut reminder| {
          f(&mut reminder);
          Some(reminder)
        });
    });
  }

  pub fn get_all_reminders(&self) -> Vec<Reminder> {
    let txn = self.container.transact();
    self
      .container
      .iter(&txn)
      .flat_map(|value| {
        if let YrsValue::Any(any) = value {
          Some(Reminder::from(any))
        } else {
          None
        }
      })
      .collect()
  }
}

fn subscribe_reminder_change(
  root: &mut ArrayRefWrapper,
  change_tx: RemindersChangeSender,
) -> DeepEventsSubscription {
  root.observe_deep(move |txn, events| {
    for deep_event in events.iter() {
      match deep_event {
        Event::Array(event) => {
          for c in event.delta(txn) {
            let _change_tx = change_tx.clone();
            match c {
              Change::Added(_) => {},
              Change::Removed(_) => {},
              Change::Retain(_) => {},
            }
          }
        },
        _ => {},
      }
    }
  })
}

const REMINDER_ID: &str = "id";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
  #[serde(rename = "id")]
  id: String,
  scheduled_at: i64,
  is_ack: bool,
  ty: i64,
  title: String,
  message: String,
  reminder_object_id: String,
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

impl From<lib0Any> for Reminder {
  fn from(any: lib0Any) -> Self {
    let mut json = String::new();
    any.to_json(&mut json);
    serde_json::from_str(&json).unwrap()
  }
}

impl From<Reminder> for lib0Any {
  fn from(item: Reminder) -> Self {
    let json = serde_json::to_string(&item).unwrap();
    lib0Any::from_json(&json).unwrap()
  }
}

pub struct ReminderUpdate<'a, 'b, 'c> {
  map_ref: &'c MapRefWrapper,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> ReminderUpdate<'a, 'b, 'c> {}
