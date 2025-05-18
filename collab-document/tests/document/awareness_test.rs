use crate::util::DocumentTest;

use collab::core::awareness::AwarenessUpdate;
use collab::preclude::block::ClientID;
use collab::preclude::updates::decoder::{Decode, Decoder};
use collab_document::document_awareness::{DocumentAwarenessState, DocumentAwarenessUser};

use arc_swap::ArcSwapOption;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, mpsc};
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::updates::encoder::{Encode, Encoder};

#[test]
fn document_awareness_test() {
  let uid = 1;
  let mut test = DocumentTest::new(uid, "1");
  let document_state = DocumentAwarenessState {
    version: 1,
    user: DocumentAwarenessUser {
      uid,
      device_id: "fake_device".to_string(),
    },
    selection: None,
    metadata: None,
    timestamp: 123,
  };

  let (tx, rx) = mpsc::channel();
  test.document.subscribe_awareness_state("test", move |a| {
    assert_eq!(a.len(), 1);
    tx.send(a.values().next().unwrap().clone()).unwrap();
  });

  test
    .document
    .set_awareness_local_state(document_state.clone());
  assert_eq!(
    test.get_awareness_local_state().as_ref(),
    Some(&document_state)
  );
  let document_state_from_awareness = rx.recv().unwrap();
  assert_eq!(
    document_state_from_awareness.version,
    document_state.version
  );
  assert_eq!(
    document_state_from_awareness.user.uid,
    document_state.user.uid
  );
  assert_eq!(
    document_state_from_awareness.user.device_id,
    document_state.user.device_id
  );
  assert_eq!(
    document_state_from_awareness.timestamp,
    document_state.timestamp
  );
}

#[test]
fn document_awareness_serde_test() {
  // This test is to reproduce the serde issue when decoding the [OldAwarenessUpdate] object with the
  // [AwarenessUpdate].
  let document_state = DocumentAwarenessState {
    version: 1,
    user: DocumentAwarenessUser {
      uid: 1,
      device_id: "fake_device".to_string(),
    },
    selection: None,
    metadata: None,
    timestamp: 123,
  };

  // Simulate decoding the [OldAwarenessUpdate] object with the [AwarenessUpdate] decoder. Check if
  // the [DocumentAwarenessState] can be decoded correctly.
  let mut old_version_awareness_update = OldAwarenessUpdate {
    clients: Default::default(),
  };
  old_version_awareness_update.clients.insert(
    1,
    OldAwarenessUpdateEntry {
      clock: 0,
      json: serde_json::to_value(&document_state).unwrap(),
    },
  );

  let new_version_awareness_update =
    AwarenessUpdate::decode_v1(&old_version_awareness_update.encode_v1()).unwrap();
  let document_state_from_new_version_awareness = serde_json::from_str::<DocumentAwarenessState>(
    &new_version_awareness_update
      .clients
      .values()
      .next()
      .unwrap()
      .json,
  )
  .unwrap();
  assert_eq!(
    document_state_from_new_version_awareness.version,
    document_state.version
  );
}

#[test]
fn document_awareness_serde_test2() {
  // This test is to reproduce the serde issue when decoding the [OldAwarenessUpdate] object with the
  // [AwarenessUpdate].
  let document_state = DocumentAwarenessState {
    version: 1,
    user: DocumentAwarenessUser {
      uid: 1,
      device_id: "fake_device".to_string(),
    },
    selection: None,
    metadata: None,
    timestamp: 123,
  };

  let mut new_version_awareness_update = AwarenessUpdate {
    clients: Default::default(),
  };
  new_version_awareness_update.clients.insert(
    1,
    AwarenessUpdateEntry {
      clock: 0,
      json: serde_json::to_string(&document_state).unwrap().into(),
    },
  );

  let old_version_awareness_update =
    OldAwarenessUpdate::decode_v1(&new_version_awareness_update.encode_v1()).unwrap();
  let document_state_from_old_version_awareness = serde_json::from_value::<DocumentAwarenessState>(
    old_version_awareness_update
      .clients
      .values()
      .next()
      .unwrap()
      .json
      .clone(),
  )
  .unwrap();
  assert_eq!(
    document_state_from_old_version_awareness.version,
    document_state.version
  );
}

#[test]
fn document_awareness_incoming_update() {
  let mut d1 = DocumentTest::new(2, "1");
  let mut d2 = DocumentTest::new(2, "1");

  // 1. subscribe to awareness state changes on both ends
  let d1_awareness_state = Arc::new(ArcSwapOption::default());
  let awareness_state = d1_awareness_state.clone();
  d1.document
    .subscribe_awareness_state("test", move |a| awareness_state.store(Some(a.into())));

  let d2_awareness_state = Arc::new(ArcSwapOption::default());
  let awareness_state = d2_awareness_state.clone();
  d2.document
    .subscribe_awareness_state("test", move |a| awareness_state.store(Some(a.into())));

  // 2. subscribe to awareness changes in the same way as `observe_awareness` from collab.rs
  let d1 = Arc::new(d1);
  let d2 = Arc::new(d2);
  let other = d2.clone();
  d1.get_awareness()
    .on_update_with("sync", move |awareness, e, _| {
      // we only send actual changes not entire awareness state
      if let Ok(update) = awareness.update_with_clients(e.all_changes()) {
        other.get_awareness().apply_update(update).unwrap();
      }
    });
  let other = d1.clone();
  d2.get_awareness()
    .on_update_with("sync", move |awareness, e, _| {
      // we only send actual changes not entire awareness state
      if let Ok(update) = awareness.update_with_clients(e.all_changes()) {
        other.get_awareness().apply_update(update).unwrap();
      }
    });

  // 3. set new awareness state on both ends
  d1.set_awareness_local_state(DocumentAwarenessState {
    version: 1,
    user: DocumentAwarenessUser {
      uid: 1,
      device_id: "device_1".to_string(),
    },
    selection: None,
    metadata: Some("meta1".into()),
    timestamp: 1111,
  });
  d2.set_awareness_local_state(DocumentAwarenessState {
    version: 1,
    user: DocumentAwarenessUser {
      uid: 2,
      device_id: "device_2".to_string(),
    },
    selection: None,
    metadata: Some("meta2".into()),
    timestamp: 2222,
  });

  // 4. compare received states
  let state1 = d1_awareness_state.swap(None).unwrap();
  let state2 = d2_awareness_state.swap(None).unwrap();

  assert_eq!(state1, state2, "both clients should have equal state");
  assert_eq!(
    state1.len(),
    2,
    "subscribe_awareness_state should return full state"
  );
}

/// the [OldAwarenessUpdate] is the object used before the [AwarenessUpdate] is introduced. In here,
/// we use the [OldAwarenessUpdate] to simulate the old awareness update object. Try to reproduce
/// serde issue when decoding the [OldAwarenessUpdate] object with the [AwarenessUpdate] decoder.
#[derive(Debug, Eq, PartialEq, Clone)]
struct OldAwarenessUpdate {
  pub(crate) clients: HashMap<ClientID, OldAwarenessUpdateEntry>,
}

impl Display for OldAwarenessUpdate {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    for client in self.clients.iter() {
      write!(f, "{}", client.1)?;
    }
    Ok(())
  }
}

impl Encode for OldAwarenessUpdate {
  fn encode<E: Encoder>(&self, encoder: &mut E) {
    encoder.write_var(self.clients.len());
    for (&client_id, e) in self.clients.iter() {
      encoder.write_var(client_id);
      encoder.write_var(e.clock);
      encoder.write_string(&e.json.to_string());
    }
  }
}

impl Decode for OldAwarenessUpdate {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, yrs::encoding::read::Error> {
    let len: usize = decoder.read_var()?;
    let mut clients = HashMap::with_capacity(len);
    for _ in 0..len {
      let client_id: ClientID = decoder.read_var()?;
      let clock: u32 = decoder.read_var()?;
      let json = serde_json::from_str(decoder.read_string()?)?;
      clients.insert(client_id, OldAwarenessUpdateEntry { clock, json });
    }

    Ok(OldAwarenessUpdate { clients })
  }
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct OldAwarenessUpdateEntry {
  clock: u32,
  json: Value,
}

impl Display for OldAwarenessUpdateEntry {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "AwarenessUpdateEntry {{ clock: {}, json: {} }}",
      self.clock, self.json
    )
  }
}
