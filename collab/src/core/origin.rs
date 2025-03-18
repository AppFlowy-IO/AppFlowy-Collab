use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use yrs::{Origin, TransactionMut};

///  ⚠️ ⚠️ ⚠️Compatibility Warning:
///
/// The structure of this struct is integral to maintaining compatibility with existing messages.
/// Therefore, adding or removing any properties (fields) from this struct could disrupt the
/// compatibility.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum CollabOrigin {
  Client(CollabClient),
  Server,
  Empty,
}

impl CollabOrigin {
  pub fn client_user_id(&self) -> Option<i64> {
    match self {
      CollabOrigin::Client(origin) => Some(origin.uid),
      CollabOrigin::Server => None,
      CollabOrigin::Empty => None,
    }
  }
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

impl FromStr for CollabOrigin {
  type Err = crate::error::CollabError;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "" => Ok(CollabOrigin::Empty),
      "server" => Ok(CollabOrigin::Server),
      other => {
        let mut split = other.split('|');
        match (split.next(), split.next()) {
          (Some(uid), Some(device_id)) | (Some(device_id), Some(uid))
            if uid.starts_with("uid:") && device_id.starts_with("device_id:") =>
          {
            let uid = uid.trim_start_matches("uid:");
            let device_id = device_id.trim_start_matches("device_id:").to_string();
            let uid: i64 = uid.parse().map_err(|err| {
              crate::error::CollabError::NoRequiredData(format!("failed to parse uid: {}", err))
            })?;
            Ok(CollabOrigin::Client(CollabClient { uid, device_id }))
          },
          _ => Err(crate::error::CollabError::NoRequiredData(format!(
            "couldn't parse collab origin from `{}`",
            other
          ))),
        }
      },
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
    serde_json::from_slice::<CollabOrigin>(value.as_ref()).unwrap_or(CollabOrigin::Empty)
  }
}

///  ⚠️ ⚠️ ⚠️Compatibility Warning:
///
/// The structure of this struct is integral to maintaining compatibility with existing messages.
/// Therefore, adding or removing any properties (fields) from this struct could disrupt the
/// compatibility.
///
/// This [CollabClient] is used to verify the origin of a [Transaction] when
/// applying a remote update.
#[derive(Serialize, Deserialize, Eq, PartialEq, Hash, Debug, Clone)]
pub struct CollabClient {
  pub uid: i64,
  pub device_id: String,
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

#[cfg(test)]
mod test {
  use crate::core::origin::{CollabClient, CollabOrigin};

  #[test]
  fn parse_collab_origin_from_empty() {
    parse_collab_origin(CollabOrigin::Empty);
  }

  #[test]
  fn parse_collab_origin_from_server() {
    parse_collab_origin(CollabOrigin::Server);
  }

  #[test]
  fn parse_collab_origin_from_client() {
    parse_collab_origin(CollabOrigin::Client(CollabClient::new(
      0xdeadbeefdeadbee,
      "device-1",
    )));
  }

  fn parse_collab_origin(origin: CollabOrigin) {
    let origin_str = origin.to_string();
    let parsed = origin_str.parse::<CollabOrigin>().unwrap();
    assert_eq!(origin, parsed);
  }
}
