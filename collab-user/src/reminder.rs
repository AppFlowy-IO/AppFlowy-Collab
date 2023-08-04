use collab::core::array_wrapper::ArrayRefExtension;
use collab::preclude::{lib0Any, ArrayRefWrapper, MapRefWrapper, TransactionMut};
use serde::{Deserialize, Serialize};

pub struct Reminders {
  pub(crate) container: ArrayRefWrapper,
}

impl Reminders {
  pub fn remove(&self, id: &str) {
    self.container.with_transact_mut(|txn| {
      self.container.remove_with_id(txn, "id", id);
    });
  }

  pub fn add(&self, reminder: Reminder) {
    self.container.push(reminder);
  }

  pub fn update_reminder<F>(&self, _reminder_id: &str, _f: F)
  where
    F: FnOnce(ReminderUpdate),
  {
    todo!()
  }
}

#[derive(Serialize, Deserialize)]
pub struct Reminder {
  id: String,
  scheduled_at: i64,
  is_ack: bool,
  ty: i64,
  title: String,
  message: String,
  reminder_object_id: String,
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
