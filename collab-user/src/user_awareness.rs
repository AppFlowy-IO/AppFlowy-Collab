use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use anyhow::Result;
use collab::core::collab::MutexCollab;
use collab::core::collab_state::SyncState;
use collab::preclude::{Any, MapPrelim, MapRefWrapper};
use collab_entity::reminder::Reminder;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;

use crate::appearance::AppearanceSettings;
use crate::reminder::{Reminders, RemindersChangeSender};

const USER: &str = "user_awareness";
const REMINDERS: &str = "reminders";
const APPEARANCE_SETTINGS: &str = "appearance_settings";

/// A thread-safe wrapper around the `UserAwareness` struct.
///
/// This structure uses an `Arc<Mutex<T>>` pattern to ensure that the underlying `UserAwareness`
/// can be safely shared and mutated across multiple threads.
#[derive(Clone)]
pub struct MutexUserAwareness(Arc<Mutex<UserAwareness>>);
impl MutexUserAwareness {
  pub fn new(inner: UserAwareness) -> Self {
    #[allow(clippy::arc_with_non_send_sync)]
    Self(Arc::new(Mutex::new(inner)))
  }
}

impl Deref for MutexUserAwareness {
  type Target = Arc<Mutex<UserAwareness>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexUserAwareness {}
unsafe impl Send for MutexUserAwareness {}

pub struct UserAwareness {
  inner: Arc<MutexCollab>,
  #[allow(dead_code)]
  container: MapRefWrapper,
  #[allow(dead_code)]
  appearance_settings: AppearanceSettings,
  reminders: Reminders,
  #[allow(dead_code)]
  notifier: Option<UserAwarenessNotifier>,
}

impl UserAwareness {
  /// Creates a new instance from a given collaboration object.
  ///
  /// This function locks the given collaboration object and performs
  /// an origin transaction. Within this transaction, several elements
  /// and structures like `awareness`, `appearance_settings`, and `reminders`
  /// are set up or fetched. These are then used to initialize and return
  /// a new instance.
  ///
  /// # Parameters
  /// - `collab`: A shared reference to a `MutexCollab` object. This object is
  ///   locked to ensure thread-safe access during the creation process.
  ///
  /// # Returns
  /// - A new instance containing references to parts of the collaboration
  ///   object like `container`, `appearance_settings`, and `reminders`.
  ///
  /// # Panics
  /// - This function might panic if it fails to lock the `collab` mutex.
  ///
  pub fn create(collab: Arc<MutexCollab>, notifier: Option<UserAwarenessNotifier>) -> Self {
    let collab_guard = collab.lock();
    let (container, appearance_settings, reminders) =
      collab_guard.with_origin_transact_mut(|txn| {
        let awareness = collab_guard.insert_map_with_txn_if_not_exist(txn, USER);

        let appearance_settings_container =
          awareness.create_map_with_txn_if_not_exist(txn, APPEARANCE_SETTINGS);
        let appearance_settings = AppearanceSettings {
          container: appearance_settings_container,
        };

        let reminder_container =
          awareness.create_array_if_not_exist_with_txn::<MapPrelim<Any>, _>(txn, REMINDERS, vec![]);
        let reminders = Reminders::new(
          reminder_container,
          notifier
            .as_ref()
            .map(|notifier| notifier.reminder_change_tx.clone()),
        );

        (awareness, appearance_settings, reminders)
      });
    drop(collab_guard);
    Self::new(collab, container, appearance_settings, reminders, notifier)
  }

  /// Provides mechanisms to manage user awareness in a collaborative context.
  ///
  /// This structure interacts with a `MutexCollab` to retrieve or create user awareness attributes,
  /// which include appearance settings and reminders.
  /// Attempts to open and retrieve existing user awareness or creates a new one if necessary.
  ///
  /// If the user awareness attributes are not present, it logs an informational message and
  /// proceeds to create them. The method encapsulates the logic to seamlessly handle existing
  /// or missing attributes, offering a single point of access.
  pub fn open(collab: Arc<MutexCollab>, notifier: Option<UserAwarenessNotifier>) -> Self {
    Self::try_open(collab.clone(), notifier.clone()).unwrap_or_else(|| {
      tracing::info!("Create missing attributes of user awareness");
      Self::create(collab, notifier)
    })
  }

  /// Constructs a new instance with the provided parameters.
  ///
  /// This private method serves as a constructor for the type, providing a
  /// centralized point for instance creation. It is called internally by other
  /// methods that need to return an instance.
  ///
  /// # Parameters
  /// - `inner`: A shared reference to the `MutexCollab` object.
  /// - `container`: A reference to the user awareness map.
  /// - `appearance_settings`: User's appearance settings.
  /// - `reminders`: User's reminders.
  /// - `notifier`: An optional notifier for user awareness changes.
  ///
  fn new(
    inner: Arc<MutexCollab>,
    container: MapRefWrapper,
    appearance_settings: AppearanceSettings,
    reminders: Reminders,
    notifier: Option<UserAwarenessNotifier>,
  ) -> Self {
    Self {
      inner,
      container,
      appearance_settings,
      reminders,
      notifier,
    }
  }

  /// Tries to retrieve user awareness attributes from the given collaboration object.
  ///
  /// This private method attempts to access existing user awareness attributes, including
  /// appearance settings and reminders. If all attributes are found, an instance is
  /// returned. Otherwise, it returns `None`.
  fn try_open(collab: Arc<MutexCollab>, notifier: Option<UserAwarenessNotifier>) -> Option<Self> {
    let collab_guard = collab.lock();
    let txn = collab_guard.transact();
    let awareness = collab_guard.get_map_with_txn(&txn, vec![USER])?;
    let appearance_settings = AppearanceSettings {
      container: awareness.get_map_with_txn(&txn, APPEARANCE_SETTINGS)?,
    };

    let reminders = Reminders::new(
      awareness.get_array_ref_with_txn(&txn, REMINDERS)?,
      notifier
        .as_ref()
        .map(|notifier| notifier.reminder_change_tx.clone()),
    );
    drop(txn);
    drop(collab_guard);
    Some(Self::new(
      collab,
      awareness,
      appearance_settings,
      reminders,
      notifier,
    ))
  }

  /// Converts the internal state of the `UserAwareness` into a JSON representation.
  ///
  /// This method constructs an instance of `UserAwarenessData` with the current data,
  /// then serializes it into a JSON value.
  pub fn to_json(&self) -> Result<serde_json::Value> {
    let data = UserAwarenessData {
      appearance_settings: Default::default(),
      reminders: self.reminders.get_all_reminders(),
    };
    let value = serde_json::to_value(data)?;
    Ok(value)
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.lock().subscribe_sync_state()
  }

  /// Adds a new reminder to the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder` - The `Reminder` object to be added.
  pub fn add_reminder(&self, reminder: Reminder) {
    self.reminders.add(reminder);
  }

  /// Returns all reminders in the `UserAwareness` object.
  pub fn get_all_reminders(&self) -> Vec<Reminder> {
    self.reminders.get_all_reminders()
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
    F: FnOnce(&mut Reminder),
  {
    self.reminders.update_reminder(reminder_id, f);
  }
}

#[derive(Clone)]
pub struct UserAwarenessNotifier {
  pub reminder_change_tx: RemindersChangeSender,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAwarenessData {
  pub appearance_settings: HashMap<String, String>,
  pub reminders: Vec<Reminder>,
}
