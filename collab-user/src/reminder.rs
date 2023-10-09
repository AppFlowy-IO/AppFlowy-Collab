use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{
  Array, ArrayRefWrapper, Change, DeepEventsSubscription, DeepObservable, Event, MapPrelim,
  YrsValue,
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
