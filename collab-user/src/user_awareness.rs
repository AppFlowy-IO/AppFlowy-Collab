use std::sync::Arc;

use crate::appearance_settings::AppearanceSettings;
use crate::reminder::{Reminder, ReminderUpdate, Reminders};

use collab::core::collab::MutexCollab;
use collab::preclude::{lib0Any, Array, ArrayRefWrapper, MapRefWrapper};

const USER: &str = "user_awareness";
const REMINDERS: &str = "reminders";
const APPEARANCE_SETTINGS: &str = "appearance_settings";

pub struct UserAwareness {
  container: MapRefWrapper,
  appearance_settings: AppearanceSettings,
  reminders: Reminders,
}

impl UserAwareness {
  pub fn create(collab: Arc<MutexCollab>) -> Self {
    let collab_guard = collab.lock();
    let (container, appearance_settings, reminders) = collab_guard.with_transact_mut(|txn| {
      let awareness = collab_guard.insert_map_with_txn_if_not_exist(txn, USER);

      let appearance_settings_container =
        awareness.insert_map_with_txn_if_not_exist(txn, APPEARANCE_SETTINGS);
      let appearance_settings = AppearanceSettings {
        container: appearance_settings_container,
      };

      let reminder_container =
        awareness.insert_array_if_not_exist_with_txn::<Reminder>(txn, REMINDERS, vec![]);
      let reminders = Reminders {
        container: reminder_container,
      };

      (awareness, appearance_settings, reminders)
    });
    Self {
      container,
      appearance_settings,
      reminders,
    }
  }

  /// Adds a new reminder to the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder` - The `Reminder` object to be added.
  pub fn add_reminder(&self, reminder: Reminder) {
    self.reminders.add(reminder);
  }

  /// Removes an existing reminder from the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder_id` - A string reference to the ID of the reminder to be removed.
  pub fn remove_reminder(&self, reminder_id: &str) {
    self.reminders.remove(reminder_id);
  }

  /// Updates an existing reminder in the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder_id` - A string reference to the ID of the reminder to be updated.
  /// * `f` - A function or closure that takes `ReminderUpdate` as its argument and implements the changes to the reminder.
  pub fn update_reminder<F>(&self, reminder_id: &str, f: F)
  where
    F: FnOnce(ReminderUpdate),
  {
    self.reminders.update_reminder(reminder_id, f);
  }
}
