use collab::preclude::encoding::serde::{from_any, to_any};
use collab::preclude::{
  Any, Array, ArrayRef, Change, DeepObservable, Event, Map, MapPrelim, ReadTxn, Subscription,
  ToJson, TransactionMut, Value, YrsValue,
};
use collab_entity::reminder::{Reminder, REMINDER_ID};
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

  fn find<T: ReadTxn>(&self, txn: &T, reminder_id: &str) -> Option<(u32, Value)> {
    let mut i = 0;
    for value in self.container.iter(txn) {
      if let Value::YMap(map) = &value {
        if let Some(Value::Any(Any::String(str))) = map.get(txn, REMINDER_ID) {
          if &*str == reminder_id {
            return Some((i, value));
          }
        }
      }
      i += 1;
    }
    None
  }

  pub fn remove(&self, txn: &mut TransactionMut, id: &str) {
    let (i, _) = self.find(txn, id).unwrap();
    self.container.remove(txn, i);
  }

  pub fn add(&self, txn: &mut TransactionMut, reminder: Reminder) {
    let map: MapPrelim<_> = reminder.into();
    self.container.push_back(txn, map);
  }

  pub fn update_reminder<F>(&self, txn: &mut TransactionMut, reminder_id: &str, f: F)
  where
    F: FnOnce(&mut Reminder),
  {
    if let Some((i, value)) = self.find(txn, reminder_id) {
      if let Ok(mut reminder) = from_any::<Reminder>(&value.to_json(txn)) {
        // FIXME: this is wrong. This doesn't "update" the reminder, instead it replaces it,
        //   with all of the unchanged fields included. That means, that if two people will be
        //   updating the same reminder at the same time, the last one will overwrite the changes
        //   of the first one, even if there was no edit collisions.
        f(&mut reminder);
        self.container.remove(txn, i);
        let any = to_any(&reminder).unwrap();
        self.container.insert(txn, i, any);
      }
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
