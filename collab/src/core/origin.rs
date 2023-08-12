use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use yrs::{Origin, TransactionMut};

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum CollabOrigin {
  Client(CollabClient),
  Server,
  Empty,
}

impl Display for CollabOrigin {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      CollabOrigin::Client(origin) => f.write_fmt(format_args!(
        "uid:{}|device_id:{}",
        origin.uid, origin.device_id,
      )),
      CollabOrigin::Server => f.write_fmt(format_args!("server")),
      CollabOrigin::Empty => Ok(()),
    }
  }
}

impl From<CollabOrigin> for Origin {
  fn from(origin: CollabOrigin) -> Self {
    let data = serde_json::to_vec(&origin).unwrap();
    Origin::from(data.as_slice())
  }
}

impl<'a> From<&TransactionMut<'a>> for CollabOrigin {
  fn from(txn: &TransactionMut<'a>) -> Self {
    match txn.origin() {
      None => CollabOrigin::Empty,
      Some(origin) => Self::from(origin),
    }
  }
}

impl From<&Origin> for CollabOrigin {
  fn from(value: &Origin) -> Self {
    match serde_json::from_slice::<CollabOrigin>(value.as_ref()) {
      Ok(origin) => origin,
      Err(_) => CollabOrigin::Empty,
    }
  }
}

/// This [CollabClient] is used to verify the origin of a [Transaction] when
/// applying a remote update.
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone)]
pub struct CollabClient {
  pub uid: i64,
  device_id: String,
}

impl Display for CollabClient {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "[uid:{}|device_id:{}]",
      self.uid, self.device_id,
    ))
  }
}

impl CollabClient {
  pub fn new(uid: i64, device_id: impl ToString) -> Self {
    let device_id = device_id.to_string();
    debug_assert!(
      !device_id.is_empty(),
      "device_id should not be empty string"
    );
    Self { uid, device_id }
  }
}

impl From<CollabClient> for Origin {
  fn from(origin: CollabClient) -> Self {
    let data = serde_json::to_vec(&origin).unwrap();
    Origin::from(data.as_slice())
  }
}
