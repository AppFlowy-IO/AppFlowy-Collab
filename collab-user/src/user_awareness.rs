use std::collections::HashMap;

use std::sync::{Arc, Mutex};

use crate::reminder::{Reminders, RemindersChangeSender};
use anyhow::Result;
use collab::core::collab_state::SyncState;
<<<<<<< HEAD
use collab::preclude::{Any, MapRefWrapper};
=======
use collab::preclude::{Collab, MapExt, MapRef};
>>>>>>> 8473e96 (draft)
use collab_entity::define::USER_AWARENESS;
use collab_entity::reminder::Reminder;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::WatchStream;

const REMINDERS: &str = "reminders";
const APPEARANCE_SETTINGS: &str = "appearance_settings";

pub struct UserAwareness {
  inner: Arc<Mutex<Collab>>,
  #[allow(dead_code)]
  container: MapRef,
  #[allow(dead_code)]
  appearance_settings: MapRef,
  reminders: Reminders,
  #[allow(dead_code)]
  notifier: Option<UserAwarenessNotifier>,
}

unsafe impl Sync for UserAwareness {}
unsafe impl Send for UserAwareness {}

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
  pub fn create(collab: Arc<Mutex<Collab>>, notifier: Option<UserAwarenessNotifier>) -> Self {
    let mut collab_guard = collab.lock().unwrap();
    let (container, appearance_settings, reminders) = {
      let collab = &mut *collab_guard;
      let mut txn = collab.context.transact_mut();
      let awareness = collab.data.get_or_init_map(&mut txn, USER_AWARENESS);

      let appearance_settings = awareness.get_or_init_map(&mut txn, APPEARANCE_SETTINGS);

      let reminder_container = awareness.get_or_init_array(&mut txn, REMINDERS);
      let reminders = Reminders::new(
        reminder_container,
        notifier
          .as_ref()
          .map(|notifier| notifier.reminder_change_tx.clone()),
      );

      (awareness, appearance_settings, reminders)
    };
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
  pub fn open(collab: Arc<Mutex<Collab>>, notifier: Option<UserAwarenessNotifier>) -> Self {
    let user_awareness = Self::try_open(collab.clone(), notifier.clone());
    match user_awareness {
      None => {
        tracing::info!("Create missing attributes of user awareness");
        Self::create(collab, notifier)
      },
      Some(user_awareness) => user_awareness,
    }
  }

  pub fn close(&self) {
    self.inner.lock().unwrap().clear_plugins();
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
    inner: Arc<Mutex<Collab>>,
    container: MapRef,
    appearance_settings: MapRef,
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
  fn try_open(collab: Arc<Mutex<Collab>>, notifier: Option<UserAwarenessNotifier>) -> Option<Self> {
    let mut collab_guard = collab.lock().unwrap();
    let collab_mut = &mut *collab_guard;
    let txn = collab_mut.context.transact();
    let awareness: MapRef = collab_mut.data.get_with_txn(&txn, USER_AWARENESS)?;
    let appearance_settings = awareness.get_with_txn(&txn, APPEARANCE_SETTINGS)?;

    let reminders = Reminders::new(
      awareness.get_with_txn(&txn, REMINDERS)?,
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
    let reminders = self.get_all_reminders();
    let data = UserAwarenessData {
      appearance_settings: Default::default(),
      reminders,
    };
    let value = serde_json::to_value(data)?;
    Ok(value)
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.lock().unwrap().subscribe_sync_state()
  }

  /// Adds a new reminder to the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder` - The `Reminder` object to be added.
  pub fn add_reminder(&self, reminder: Reminder) {
    let mut lock = self.inner.lock().unwrap();
    self
      .reminders
      .add(&mut lock.context.transact_mut(), reminder);
  }

  /// Returns all reminders in the `UserAwareness` object.
  pub fn get_all_reminders(&self) -> Vec<Reminder> {
    let lock = self.inner.lock().unwrap();
    let txn = lock.transact();
    self.reminders.get_all_reminders(&txn)
  }

  /// Removes an existing reminder from the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder_id` - A string reference to the ID of the reminder to be removed.
  pub fn remove_reminder(&self, reminder_id: &str) {
    let mut lock = self.inner.lock().unwrap();
    self
      .reminders
      .remove(&mut lock.context.transact_mut(), reminder_id);
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
    let mut lock = self.inner.lock().unwrap();
    self
      .reminders
      .update_reminder(&mut lock.context.transact_mut(), reminder_id, f);
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
