use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

use crate::core::ReminderUpdate;
use crate::reminder::{Reminders, RemindersChangeSender};
use anyhow::{Error, Result};
use collab::core::collab::CollabOptions;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::block::ClientID;
use collab::preclude::{ArrayRef, Collab, Map, MapExt, MapRef};
use collab_entity::CollabType;
use collab_entity::define::USER_AWARENESS;
use collab_entity::reminder::Reminder;
use serde::{Deserialize, Serialize};

const REMINDERS: &str = "reminders";
const APPEARANCE_SETTINGS: &str = "appearance_settings";

pub struct UserAwareness {
  collab: Collab,
  body: UserAwarenessBody,
}

impl UserAwareness {
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
  fn new(collab: Collab, body: UserAwarenessBody) -> Self {
    Self { collab, body }
  }

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
  pub fn open(mut collab: Collab, notifier: Option<UserAwarenessNotifier>) -> Result<Self, Error> {
    CollabType::UserAwareness.validate_require_data(&collab)?;
    let body = UserAwarenessBody::new(&mut collab, notifier);
    Ok(Self::new(collab, body))
  }

  pub fn create(
    mut collab: Collab,
    notifier: Option<UserAwarenessNotifier>,
  ) -> Result<Self, Error> {
    let body = UserAwarenessBody::new(&mut collab, notifier);
    Ok(Self::new(collab, body))
  }

  /// Tries to retrieve user awareness attributes from the given collaboration object.
  ///
  /// This private method attempts to access existing user awareness attributes, including
  /// appearance settings and reminders. If all attributes are found, an instance is
  /// returned. Otherwise, it returns `None`.
  pub fn try_open(collab: Collab, notifier: Option<UserAwarenessNotifier>) -> Option<Self> {
    let body = UserAwarenessBody::try_open(&collab, notifier)?;
    Some(Self::new(collab, body))
  }

  pub fn close(&self) {
    self.collab.remove_all_plugins();
  }

  /// Converts the internal state of the `UserAwareness` into a JSON representation.
  ///
  /// This method constructs an instance of `UserAwarenessData` with the current data,
  /// then serializes it into a JSON value.
  pub fn to_json(&self) -> Result<serde_json::Value> {
    let txn = self.collab.transact();
    let reminders = self.body.reminders.get_all_reminders(&txn);
    let data = UserAwarenessData {
      appearance_settings: Default::default(),
      reminders,
    };
    let value = serde_json::to_value(data)?;
    Ok(value)
  }

  pub fn get_all_reminders(&self) -> Vec<Reminder> {
    let txn = self.collab.transact();
    self.body.reminders.get_all_reminders(&txn)
  }

  /// Adds a new reminder to the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder` - The `Reminder` object to be added.
  pub fn add_reminder(&mut self, reminder: Reminder) {
    let mut txn = self.collab.transact_mut();
    self.body.reminders.add(&mut txn, reminder);
  }

  /// Removes an existing reminder from the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder_id` - A string reference to the ID of the reminder to be removed.
  pub fn remove_reminder(&mut self, reminder_id: &str) {
    let mut txn = self.collab.transact_mut();
    self.body.reminders.remove(&mut txn, reminder_id);
  }

  /// Updates an existing reminder in the `UserAwareness` object.
  ///
  /// # Arguments
  ///
  /// * `reminder_id` - A string reference to the ID of the reminder to be updated.
  /// * `f` - A function or closure that takes `ReminderUpdate` as its argument and implements the changes to the reminder.
  pub fn update_reminder<F>(&mut self, reminder_id: &str, f: F)
  where
    F: FnOnce(ReminderUpdate),
  {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .reminders
      .update_reminder(&mut txn, reminder_id, f);
  }
}

pub fn default_user_awareness_data(object_id: &str, client_id: ClientID) -> EncodedCollab {
  let options = CollabOptions::new(object_id.to_string(), client_id);
  let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
  let awareness = UserAwareness::create(collab, None).unwrap();
  awareness
    .encode_collab_v1(|_collab| Ok::<_, Error>(()))
    .unwrap()
}

impl Deref for UserAwareness {
  type Target = Collab;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}

impl DerefMut for UserAwareness {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.collab
  }
}

impl Borrow<Collab> for UserAwareness {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for UserAwareness {
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
  }
}

pub struct UserAwarenessBody {
  #[allow(dead_code)]
  container: MapRef,
  #[allow(dead_code)]
  appearance_settings: MapRef,
  reminders: Reminders,
  #[allow(dead_code)]
  notifier: Option<UserAwarenessNotifier>,
}

impl UserAwarenessBody {
  pub fn new(collab: &mut Collab, notifier: Option<UserAwarenessNotifier>) -> Self {
    let mut txn = collab.context.transact_mut();
    let container = collab.data.get_or_init_map(&mut txn, USER_AWARENESS);

    let appearance_settings = container.get_or_init_map(&mut txn, APPEARANCE_SETTINGS);

    let reminder_container: ArrayRef = container.get_or_init(&mut txn, REMINDERS);
    let reminders = Reminders::new(
      reminder_container,
      notifier
        .as_ref()
        .map(|notifier| notifier.reminder_change_tx.clone()),
    );
    Self {
      container,
      appearance_settings,
      reminders,
      notifier,
    }
  }

  pub fn try_open(collab: &Collab, notifier: Option<UserAwarenessNotifier>) -> Option<Self> {
    let txn = collab.context.transact();
    let awareness: MapRef = collab.data.get_with_txn(&txn, USER_AWARENESS)?;
    let appearance_settings = awareness.get_with_txn(&txn, APPEARANCE_SETTINGS)?;

    let reminders = Reminders::new(
      awareness.get_with_txn(&txn, REMINDERS)?,
      notifier
        .as_ref()
        .map(|notifier| notifier.reminder_change_tx.clone()),
    );
    Some(Self {
      container: awareness,
      appearance_settings,
      reminders,
      notifier,
    })
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
