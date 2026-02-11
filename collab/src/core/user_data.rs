use crate::core::origin::CollabOrigin;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use yrs::block::ClientID;
use yrs::types::Change;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
  Any, Array, ArrayPrelim, ArrayRef, DeleteSet, Doc, ID, Map, MapPrelim, MapRef, Observable,
  Origin, Out, Snapshot, Subscription, Transact, TransactionMut,
};

/// Unique description of a user.
pub type UserDescription = Arc<str>;

#[derive(Default)]
struct State {
  clients: HashMap<ClientID, UserDescription>,
  dss: HashMap<UserDescription, DeleteSet>,
  action_queue: Vec<Action>,
  current_users: Vec<UserDescription>,
}

/// Permanent user data struct that keeps track of user descriptor and their associated
/// client ids (used for keeping track of who inserted a new data) and delete sets (used for
/// keeping track of who deleted what).
pub struct PermanentUserData {
  users: MapRef,
  state: Arc<parking_lot::RwLock<State>>,

  #[allow(dead_code)]
  on_users_changed: Subscription,
  #[allow(dead_code)]
  on_after_transaction: Subscription,
}

impl PermanentUserData {
  pub fn new(doc: &Doc, local_origin: CollabOrigin) -> Self {
    let users = doc.get_or_insert_map("users");
    let users_clone = users.clone();

    // we use parking_lot Mutex here because it is faster and this operation doesn't really contend
    // for lock access - it's just workaround for sharing data between observer callbacks
    let state = Arc::new(parking_lot::RwLock::new(State::default()));

    let s = state.clone();
    let on_users_changed = users.observe(move |tx, e| {
      let mut lock = s.write();
      for key in e.keys(tx).keys() {
        if let Some(Out::YMap(value)) = users_clone.get(tx, key) {
          lock.action_queue.push(Action::InitUser(key.clone(), value));
        }
      }
    });

    let client_id = doc.client_id();
    let uid: Option<Arc<str>> = if let CollabOrigin::Client(c) = &local_origin {
      Some(c.uid.to_string().into())
    } else {
      None
    };

    let local_origin: Origin = local_origin.into();
    let s = state.clone();
    let users_clone = users.clone();
    let on_after_transaction = doc
      .observe_after_transaction(move |txn| {
        let actions = std::mem::take(&mut s.write().action_queue);
        for action in actions {
          match action {
            Action::InitUser(description, user) => {
              Self::init_user(txn, s.clone(), description, user);
            },
            Action::UserOverridden(description) => {
              Self::override_user(txn, users_clone.clone(), s.clone(), description);
            },
          }
        }

        // if transaction was local add delete set to current user's ds array
        let has_deletes = !txn.delete_set().is_empty();
        let has_inserts = txn.after_state() != txn.before_state();
        if txn.origin() == Some(&local_origin) && has_deletes || has_inserts {
          // the transaction originates locally and it made some writes

          if let Some(uid) = &uid {
            // check if we already defined the current user
            let mut lock = s.write();
            if let Entry::Vacant(e) = lock.clients.entry(client_id) {
              e.insert(uid.clone());
              drop(lock);
              Self::map_user_internal(users_clone.clone(), s.clone(), txn, client_id, uid.clone());
            }
          }

          if has_deletes {
            // store new deletes info in permanent user data part of the document
            let encoded_ds = txn.delete_set().encode_v1();
            let lock = s.read();
            for user_description in &lock.current_users {
              let user: MapRef = users_clone
                .get(txn, user_description)
                .unwrap()
                .cast()
                .unwrap();
              let yds: ArrayRef = user.get(txn, "ds").unwrap().cast().unwrap();
              yds.push_back(txn, encoded_ds.clone());
            }
          }
        }
      })
      .unwrap();

    let mut tx = doc.transact_mut();

    // initialize existing users
    {
      let to_add: Vec<_> = users
        .iter(&tx)
        .flat_map(|(k, v)| match v {
          Out::YMap(map_ref) => Some((UserDescription::from(k), map_ref.clone())),
          _ => None,
        })
        .collect();

      for (description, user) in to_add {
        Self::init_user(&mut tx, state.clone(), description, user);
      }
    }
    drop(tx);

    Self {
      state,
      users,
      on_users_changed,
      on_after_transaction,
    }
  }

  fn override_user(
    tx: &mut TransactionMut,
    users: MapRef,
    state: Arc<parking_lot::RwLock<State>>,
    description: UserDescription,
  ) {
    // user was overwritten, port all data over to the next user object
    let user: MapRef = users.get(tx, &description).unwrap().cast().unwrap();
    let ds: ArrayRef = user.get(tx, "ds").unwrap().cast().unwrap();
    let ids: ArrayRef = user.get(tx, "ids").unwrap().cast().unwrap();

    let lock = state.read();
    for (old_client_id, old_description) in lock.clients.iter() {
      if *old_description == description {
        ids.push_back(tx, Any::BigInt(*old_client_id as i64));
      }
    }
    if let Some(old_ds) = lock.dss.get(&description) {
      let encoded_ds = old_ds.encode_v1();
      ds.push_back(tx, encoded_ds);
    }
  }

  fn init_user(
    tx: &mut TransactionMut,
    state: Arc<parking_lot::RwLock<State>>,
    description: UserDescription,
    user: MapRef,
  ) {
    let ds: ArrayRef = user.get(tx, "ds").unwrap().cast().unwrap();
    let ids: ArrayRef = user.get(tx, "ids").unwrap().cast().unwrap();

    // observe changes in current user's delete set array and squash them into single delete set
    // when they appear
    let user_description = description.clone();
    let s = state.clone();
    ds.observe_with("init_user", move |tx, e| {
      let mut lock = s.write();
      for delta in e.delta(tx) {
        if let Change::Added(items) = delta {
          for item in items {
            if let Out::Any(Any::Buffer(encoded_ds)) = item {
              let decoded_ds = DeleteSet::decode_v1(encoded_ds).unwrap();
              let ds = lock.dss.entry(user_description.clone()).or_default();
              ds.merge(decoded_ds);
              ds.squash();
            }
          }
        }
      }
    });

    // observe changes in current user's ids array and add them to client map
    let user_description = description.clone();
    let s = state.clone();
    ids.observe_with("init_user", move |tx, e| {
      let mut lock = s.write();
      for delta in e.delta(tx) {
        if let Change::Added(items) = delta {
          for item in items {
            if let Out::Any(Any::BigInt(id)) = item {
              lock
                .clients
                .insert(*id as ClientID, user_description.clone());
            }
          }
        }
      }
    });

    // add all existing client ids and delete sets to state
    let mut state = state.write();

    for id in ids.iter(tx) {
      let id = match id {
        Out::Any(Any::BigInt(id)) => id as ClientID,
        Out::Any(Any::Number(id)) => id as ClientID,
        _ => continue,
      };
      state.clients.insert(id as ClientID, description.clone());
    }

    let user_ds = state.dss.entry(description.clone()).or_default();
    for encoded_ds in ds.iter(tx) {
      if let Out::Any(Any::Buffer(encoded_ds)) = encoded_ds {
        let decoded_ds = DeleteSet::decode_v1(&encoded_ds).unwrap();
        user_ds.merge(decoded_ds);
      }
    }
    user_ds.squash();
  }

  /// Add mapping from client id to user description.
  pub fn map_user<S: Into<UserDescription>>(
    &mut self,
    tx: &mut TransactionMut,
    client_id: ClientID,
    description: S,
  ) {
    Self::map_user_internal(
      self.users.clone(),
      self.state.clone(),
      tx,
      client_id,
      description.into(),
    );
  }

  fn map_user_internal(
    users: MapRef,
    state: Arc<parking_lot::RwLock<State>>,
    tx: &mut TransactionMut,
    client_id: ClientID,
    user_description: UserDescription,
  ) {
    let user = match users.get(tx, &user_description) {
      Some(Out::YMap(value)) => value,
      _ => users.insert(
        tx,
        user_description.clone(),
        MapPrelim::from([
          ("ids", ArrayPrelim::default()),
          ("ds", ArrayPrelim::default()),
        ]),
      ),
    };
    let ids: ArrayRef = user.get(tx, "ids").unwrap().cast().unwrap();
    ids.push_back(tx, Any::BigInt(client_id as i64));

    // check if current user was overridden
    let description_clone = user_description.clone();
    let weak_state = Arc::downgrade(&state);
    let users_clone = users.clone();
    users.observe_with("pud", move |txn, _| {
      let old_user = users_clone.get(txn, &description_clone);
      if old_user != Some(Out::YMap(user.clone())) {
        // user was overridden
        if let Some(state) = weak_state.upgrade() {
          let mut lock = state.write();
          lock
            .action_queue
            .push(Action::UserOverridden(description_clone.clone()));
        }
      }
    });

    // keep track of current user
    state.write().current_users.push(user_description);
  }

  /// Get user description by client id.
  pub fn user_by_client_id(&self, client_id: ClientID) -> Option<UserDescription> {
    let lock = self.state.read();
    lock.clients.get(&client_id).cloned()
  }

  /// Get user description by deleted block id.
  pub fn user_by_deleted_id(&self, id: &yrs::ID) -> Option<UserDescription> {
    let lock = self.state.read();
    for (description, ds) in &lock.dss {
      if ds.is_deleted(id) {
        return Some(description.clone());
      }
    }
    None
  }

  /// Return set of users that made edits between two snapshots.
  pub fn editors_between(&self, from: &Snapshot, to: &Snapshot) -> HashSet<UserDescription> {
    let mut result = HashSet::new();
    let lock = self.state.read();

    // get client ids that have changes between from and to snapshots
    for (client_id, &to_clock) in to.state_map.iter() {
      let from_clock = from.state_map.get(client_id);
      if to_clock > from_clock {
        if let Some(user) = lock.clients.get(client_id) {
          result.insert(user.clone());
        }
      }
    }

    // also check deleted ids
    for (user, ds) in lock.dss.iter() {
      if result.contains(user) {
        continue; // we already have that user
      }
      // pick the shared part between current user and `to` delete set
      let intersect = ds.intersect(&to.delete_set);
      if !intersect.is_empty() && !intersect.subset_of(&from.delete_set) {
        // if the shared part doesn't fully belong to `from` delete set, it means that there were
        // some deletes made by this user that fit into the window between from-to
        result.insert(user.clone());
      }
    }

    result
  }
}

trait DeleteSetExt {
  fn intersect(&self, other: &Self) -> Self;
  fn subset_of(&self, other: &Self) -> bool;
}

impl DeleteSetExt for DeleteSet {
  fn intersect(&self, other: &Self) -> Self {
    let mut result = DeleteSet::new();
    for (client, ranges) in self.iter() {
      if let Some(other_ranges) = other.range(client) {
        for a in ranges.iter() {
          for b in other_ranges.iter() {
            if a.start <= b.end && a.end >= b.start {
              // there's an intersection
              let start = a.start.max(b.start);
              let len = a.end.min(b.end) - start;
              result.insert(ID::new(*client, start), len);
            }
          }
        }
      }
    }
    result
  }

  fn subset_of(&self, other: &Self) -> bool {
    for (client_id, ranges) in self.iter() {
      match other.range(client_id) {
        None => return false,
        Some(other_ranges) if !ranges.subset_of(other_ranges) => return false,
        _ => { /* continue */ },
      }
    }
    true
  }
}

#[derive(Debug)]
enum Action {
  InitUser(UserDescription, MapRef),
  UserOverridden(UserDescription),
}

#[cfg(test)]
mod test {
  use crate::core::collab::{CollabOptions, default_client_id};
  use crate::core::origin::{CollabClient, CollabOrigin};
  use crate::document::{BlockType, Document, DocumentData, DocumentMeta, generate_id};
  use crate::preclude::{Collab, Doc, PermanentUserData};
  use std::collections::{HashMap, HashSet};
  use std::sync::Arc;
  use uuid::Uuid;
  use yrs::types::ToJson;
  use yrs::updates::decoder::Decode;
  use yrs::{ReadTxn, Snapshot, StateVector, Text, Transact, Update, any};

  #[test]
  fn add_or_remove_user_mappings() {
    let origin1 = CollabOrigin::Client(CollabClient::new(1, "device-A"));
    let origin2 = CollabOrigin::Client(CollabClient::new(2, "device-B"));
    let d1 = Doc::new();
    let d2 = Doc::new();
    let mut pud1 = PermanentUserData::new(&d1, origin1);
    let mut pud2 = PermanentUserData::new(&d2, origin2);

    pud1.map_user(&mut d1.transact_mut(), d1.client_id(), "user a");
    pud2.map_user(&mut d2.transact_mut(), d2.client_id(), "user b");

    let txt1 = d1.get_or_insert_text("text");
    let txt2 = d2.get_or_insert_text("text");

    txt1.insert(&mut d1.transact_mut(), 0, "xhi");
    txt1.remove_range(&mut d1.transact_mut(), 0, 1);
    txt2.insert(&mut d2.transact_mut(), 0, "hxxi");
    txt2.remove_range(&mut d2.transact_mut(), 1, 2);

    exchange_updates([&d1, &d2]);

    // now sync a third doc with same name as doc1 and then create PermanentUserData
    let d3 = Doc::new();

    exchange_updates([&d1, &d3]);

    let origin3 = CollabOrigin::Client(CollabClient::new(1, "device-C"));
    let mut pud3 = PermanentUserData::new(&d3, origin3);
    pud3.map_user(&mut d3.transact_mut(), d3.client_id(), "user a");

    exchange_updates([&d1, &d2, &d3]);

    let user1 = pud1.user_by_client_id(d1.client_id()).unwrap();
    let user2 = pud1.user_by_client_id(d2.client_id()).unwrap();
    let user3 = pud1.user_by_client_id(d3.client_id()).unwrap();

    assert_eq!(&*user1, "user a");
    assert_eq!(&*user2, "user b");
    assert_eq!(&*user3, "user a");
  }

  #[test]
  fn editors_between() {
    let origin1 = CollabOrigin::Client(CollabClient::new(1, "device-A"));
    let origin2 = CollabOrigin::Client(CollabClient::new(2, "device-B"));
    let d1 = Doc::new();
    let d2 = Doc::new();
    let mut pud1 = PermanentUserData::new(&d1, origin1);
    let mut pud2 = PermanentUserData::new(&d2, origin2);

    pud1.map_user(&mut d1.transact_mut(), d1.client_id(), "user a");
    pud2.map_user(&mut d2.transact_mut(), d2.client_id(), "user b");

    let txt1 = d1.get_or_insert_text("text");
    let txt2 = d2.get_or_insert_text("text");

    let snap1 = {
      let mut t1 = d1.transact_mut();
      txt1.insert(&mut t1, 0, "hello world");
      t1.snapshot()
    };

    exchange_updates([&d1, &d2]);

    let users = pud1.editors_between(&Snapshot::default(), &snap1);
    assert_eq!(users, HashSet::from(["user a".into()]));

    let snap2 = {
      let mut t2 = d2.transact_mut();
      txt2.remove_range(&mut t2, 4, 3); // remove "o w"
      t2.snapshot()
    };

    exchange_updates([&d1, &d2]);

    let users = pud1.editors_between(&snap1, &snap2);
    assert_eq!(users, HashSet::from(["user b".into()]));

    let users = pud1.editors_between(&Snapshot::default(), &snap2);
    assert_eq!(users, HashSet::from(["user a".into(), "user b".into()]));
  }

  fn exchange_updates<const N: usize>(docs: [&Doc; N]) {
    let updates: Vec<_> = docs
      .iter()
      .map(|d| {
        d.transact()
          .encode_state_as_update_v1(&StateVector::default())
      })
      .collect();
    for (i, d) in docs.iter().enumerate() {
      for (j, u) in updates.iter().enumerate() {
        if i != j {
          d.transact_mut()
            .apply_update(Update::decode_v1(u).unwrap())
            .unwrap();
        }
      }
    }
  }

  #[test]
  fn collab_fills_user_data_automatically() {
    let uid = 1;
    let client_id = default_client_id();
    let oid = Uuid::new_v4();
    let origin = CollabOrigin::Client(CollabClient::new(uid, "device-1"));
    let options = CollabOptions::new(oid, client_id).with_remember_user(true);
    let collab = Collab::new_with_options(origin, options).unwrap();
    let page_id = generate_id();
    let mut document = Document::create_with_data(
      collab,
      DocumentData {
        page_id: page_id.clone(),
        blocks: HashMap::from([(
          page_id.clone(),
          crate::document::Block {
            id: page_id.clone(),
            ty: BlockType::Page.to_string(),
            parent: "".to_string(),
            children: "".to_string(),
            external_id: None,
            external_type: None,
            data: Default::default(),
          },
        )]),
        meta: DocumentMeta {
          children_map: Default::default(),
          text_map: Some(HashMap::default()),
        },
      },
    )
    .unwrap();
    document.initialize();

    let users = document.user_data().unwrap();
    assert_eq!(
      users.user_by_client_id(client_id).unwrap(),
      uid.to_string().into()
    );
  }

  #[test]
  fn collab_doesnt_fill_user_data_automatically_if_no_data_was_written() {
    let uid = 1;
    let client_id = default_client_id();
    let oid = Uuid::new_v4();
    let origin = CollabOrigin::Client(CollabClient::new(uid, "device-1"));
    let options = CollabOptions::new(oid, client_id).with_remember_user(true);
    let mut collab = Collab::new_with_options(origin, options).unwrap();

    // we use mutable transaction but we don't write anything
    let json = collab.data.to_json(&collab.context.transact_mut());
    assert_eq!(json, any!({}));

    let pud = collab.user_data().unwrap();
    assert!(pud.user_by_client_id(client_id).is_none());
  }

  #[test]
  fn client_id_from_any_number() {
    // example from test server environment
    let from = Snapshot::default();
    let to =
      Snapshot::decode_v1(&[0, 2, 158, 151, 232, 252, 6, 26, 250, 195, 249, 141, 6, 18]).unwrap();
    let update = Update::decode_v1(&[
      2, 26, 158, 151, 232, 252, 6, 0, 39, 1, 4, 100, 97, 116, 97, 8, 100, 111, 99, 117, 109, 101,
      110, 116, 1, 39, 0, 158, 151, 232, 252, 6, 0, 6, 98, 108, 111, 99, 107, 115, 1, 39, 0, 158,
      151, 232, 252, 6, 0, 4, 109, 101, 116, 97, 1, 39, 0, 158, 151, 232, 252, 6, 2, 12, 99, 104,
      105, 108, 100, 114, 101, 110, 95, 109, 97, 112, 1, 39, 0, 158, 151, 232, 252, 6, 2, 8, 116,
      101, 120, 116, 95, 109, 97, 112, 1, 40, 0, 158, 151, 232, 252, 6, 0, 7, 112, 97, 103, 101,
      95, 105, 100, 1, 119, 36, 56, 49, 102, 57, 48, 56, 52, 98, 45, 57, 97, 53, 54, 45, 53, 100,
      102, 52, 45, 56, 100, 57, 49, 45, 53, 100, 51, 57, 55, 48, 54, 101, 50, 54, 48, 54, 39, 0,
      158, 151, 232, 252, 6, 1, 10, 53, 52, 100, 66, 95, 69, 72, 102, 56, 57, 1, 40, 0, 158, 151,
      232, 252, 6, 6, 2, 105, 100, 1, 119, 10, 53, 52, 100, 66, 95, 69, 72, 102, 56, 57, 40, 0,
      158, 151, 232, 252, 6, 6, 2, 116, 121, 1, 119, 9, 112, 97, 114, 97, 103, 114, 97, 112, 104,
      40, 0, 158, 151, 232, 252, 6, 6, 6, 112, 97, 114, 101, 110, 116, 1, 119, 36, 56, 49, 102, 57,
      48, 56, 52, 98, 45, 57, 97, 53, 54, 45, 53, 100, 102, 52, 45, 56, 100, 57, 49, 45, 53, 100,
      51, 57, 55, 48, 54, 101, 50, 54, 48, 54, 40, 0, 158, 151, 232, 252, 6, 6, 8, 99, 104, 105,
      108, 100, 114, 101, 110, 1, 119, 10, 78, 117, 103, 109, 69, 77, 112, 89, 104, 66, 40, 0, 158,
      151, 232, 252, 6, 6, 4, 100, 97, 116, 97, 1, 119, 2, 123, 125, 40, 0, 158, 151, 232, 252, 6,
      6, 11, 101, 120, 116, 101, 114, 110, 97, 108, 95, 105, 100, 1, 119, 10, 99, 79, 53, 122, 74,
      89, 79, 56, 51, 65, 40, 0, 158, 151, 232, 252, 6, 6, 13, 101, 120, 116, 101, 114, 110, 97,
      108, 95, 116, 121, 112, 101, 1, 119, 4, 116, 101, 120, 116, 39, 0, 158, 151, 232, 252, 6, 3,
      10, 78, 117, 103, 109, 69, 77, 112, 89, 104, 66, 0, 39, 0, 158, 151, 232, 252, 6, 1, 36, 56,
      49, 102, 57, 48, 56, 52, 98, 45, 57, 97, 53, 54, 45, 53, 100, 102, 52, 45, 56, 100, 57, 49,
      45, 53, 100, 51, 57, 55, 48, 54, 101, 50, 54, 48, 54, 1, 40, 0, 158, 151, 232, 252, 6, 15, 2,
      105, 100, 1, 119, 36, 56, 49, 102, 57, 48, 56, 52, 98, 45, 57, 97, 53, 54, 45, 53, 100, 102,
      52, 45, 56, 100, 57, 49, 45, 53, 100, 51, 57, 55, 48, 54, 101, 50, 54, 48, 54, 40, 0, 158,
      151, 232, 252, 6, 15, 2, 116, 121, 1, 119, 4, 112, 97, 103, 101, 40, 0, 158, 151, 232, 252,
      6, 15, 6, 112, 97, 114, 101, 110, 116, 1, 119, 0, 40, 0, 158, 151, 232, 252, 6, 15, 8, 99,
      104, 105, 108, 100, 114, 101, 110, 1, 119, 36, 56, 49, 102, 57, 48, 56, 52, 98, 45, 57, 97,
      53, 54, 45, 53, 100, 102, 52, 45, 56, 100, 57, 49, 45, 53, 100, 51, 57, 55, 48, 54, 101, 50,
      54, 48, 54, 40, 0, 158, 151, 232, 252, 6, 15, 4, 100, 97, 116, 97, 1, 119, 2, 123, 125, 40,
      0, 158, 151, 232, 252, 6, 15, 11, 101, 120, 116, 101, 114, 110, 97, 108, 95, 105, 100, 1,
      126, 40, 0, 158, 151, 232, 252, 6, 15, 13, 101, 120, 116, 101, 114, 110, 97, 108, 95, 116,
      121, 112, 101, 1, 126, 39, 0, 158, 151, 232, 252, 6, 3, 36, 56, 49, 102, 57, 48, 56, 52, 98,
      45, 57, 97, 53, 54, 45, 53, 100, 102, 52, 45, 56, 100, 57, 49, 45, 53, 100, 51, 57, 55, 48,
      54, 101, 50, 54, 48, 54, 0, 8, 0, 158, 151, 232, 252, 6, 23, 1, 119, 10, 53, 52, 100, 66, 95,
      69, 72, 102, 56, 57, 39, 0, 158, 151, 232, 252, 6, 4, 10, 99, 79, 53, 122, 74, 89, 79, 56,
      51, 65, 2, 16, 250, 195, 249, 141, 6, 0, 39, 0, 158, 151, 232, 252, 6, 1, 8, 87, 76, 51, 95,
      80, 103, 97, 106, 1, 40, 0, 250, 195, 249, 141, 6, 0, 2, 105, 100, 1, 119, 8, 87, 76, 51, 95,
      80, 103, 97, 106, 40, 0, 250, 195, 249, 141, 6, 0, 2, 116, 121, 1, 119, 9, 112, 97, 114, 97,
      103, 114, 97, 112, 104, 40, 0, 250, 195, 249, 141, 6, 0, 8, 99, 104, 105, 108, 100, 114, 101,
      110, 1, 119, 8, 87, 76, 51, 95, 80, 103, 97, 106, 40, 0, 250, 195, 249, 141, 6, 0, 4, 100,
      97, 116, 97, 1, 119, 2, 123, 125, 39, 0, 158, 151, 232, 252, 6, 3, 8, 87, 76, 51, 95, 80,
      103, 97, 106, 0, 40, 0, 250, 195, 249, 141, 6, 0, 11, 101, 120, 116, 101, 114, 110, 97, 108,
      95, 105, 100, 1, 119, 8, 87, 76, 51, 95, 80, 103, 97, 106, 40, 0, 250, 195, 249, 141, 6, 0,
      13, 101, 120, 116, 101, 114, 110, 97, 108, 95, 116, 121, 112, 101, 1, 119, 4, 116, 101, 120,
      116, 39, 0, 158, 151, 232, 252, 6, 4, 8, 87, 76, 51, 95, 80, 103, 97, 106, 2, 40, 0, 250,
      195, 249, 141, 6, 0, 6, 112, 97, 114, 101, 110, 116, 1, 119, 36, 56, 49, 102, 57, 48, 56, 52,
      98, 45, 57, 97, 53, 54, 45, 53, 100, 102, 52, 45, 56, 100, 57, 49, 45, 53, 100, 51, 57, 55,
      48, 54, 101, 50, 54, 48, 54, 72, 158, 151, 232, 252, 6, 24, 1, 119, 8, 87, 76, 51, 95, 80,
      103, 97, 106, 39, 1, 5, 117, 115, 101, 114, 115, 18, 53, 53, 55, 56, 53, 56, 49, 56, 55, 55,
      51, 49, 48, 49, 51, 54, 48, 48, 1, 39, 0, 250, 195, 249, 141, 6, 11, 3, 105, 100, 115, 0, 39,
      0, 250, 195, 249, 141, 6, 11, 2, 100, 115, 0, 8, 0, 250, 195, 249, 141, 6, 12, 1, 125, 186,
      135, 243, 155, 12, 4, 0, 250, 195, 249, 141, 6, 8, 3, 97, 97, 97, 0,
    ])
    .unwrap();
    let doc = Doc::new();
    doc.transact_mut().apply_update(update).unwrap();
    let pud = PermanentUserData::new(&doc, CollabOrigin::Server);

    let editors = pud.editors_between(&from, &to);
    assert_eq!(editors, HashSet::from([Arc::from("557858187731013600")]));
  }
}
