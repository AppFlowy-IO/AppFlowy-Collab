use crate::core::origin::CollabOrigin;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
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
  clients: HashMap<u64, UserDescription>,
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
        let ds = txn.delete_set();
        if txn.origin() == Some(&local_origin) && !ds.is_empty() {
          let encoded_ds = ds.encode_v1();
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
      if let Out::Any(Any::BigInt(id)) = id {
        state.clients.insert(id as ClientID, description.clone());
      }
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
    let user_description = description.into();
    let user = match self.users.get(tx, &user_description) {
      Some(Out::YMap(value)) => value,
      _ => self.users.insert(
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
    let state = self.state.clone();
    let users = self.users.clone();
    let description_clone = user_description.clone();
    self.users.observe_with("pud", move |txn, _| {
      let old_user = users.get(txn, &description_clone);
      if old_user != Some(Out::YMap(user.clone())) {
        // user was overridden
        let mut lock = state.write();
        lock
          .action_queue
          .push(Action::UserOverridden(description_clone.clone()));
      }
    });

    // keep track of current user
    self.state.write().current_users.push(user_description);
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

    // get client ids that have changes between from and to snapshots
    for (client_id, &to_clock) in to.state_map.iter() {
      let from_clock = from.state_map.get(client_id);
      if to_clock > from_clock {
        if let Some(user) = self.user_by_client_id(*client_id) {
          result.insert(user);
        }
      }
    }

    // also check deleted ids
    //TODO: this is not very efficient, consider optimizing if needed
    let ds_diff = diff_delete_sets(&from.delete_set, &to.delete_set);
    for (client_id, ranges) in ds_diff.iter() {
      for range in ranges.iter() {
        for clock in range.start..range.end {
          let id = ID::new(*client_id, clock);
          if let Some(user) = self.user_by_deleted_id(&id) {
            result.insert(user);
          }
        }
      }
    }

    result
  }
}

fn diff_delete_sets(old_ds: &DeleteSet, new_ds: &DeleteSet) -> DeleteSet {
  let mut diff_ds = DeleteSet::new();

  for (client_id, new_range) in new_ds.iter() {
    let old_range = old_ds
      .range(client_id)
      .unwrap_or(&Default::default())
      .clone();
    let mut old_iter = old_range.iter();

    for new_range in new_range.iter() {
      if let Some(old_range) = old_iter.next() {
        if intersects(new_range, old_range) {
          // overlapping ranges, need to check for new deletions
          if new_range.start < old_range.start {
            // new deletion before old range
            diff_ds.insert(
              ID::new(*client_id, new_range.start),
              old_range.start - new_range.start,
            );
          }
          if new_range.end > old_range.end {
            // new deletion after old range
            diff_ds.insert(
              ID::new(*client_id, old_range.end),
              new_range.end - old_range.end,
            );
          }
        } else if new_range.end <= old_range.start {
          // new deletion before old range
          diff_ds.insert(
            ID::new(*client_id, new_range.start),
            new_range.end - new_range.start,
          );
        } else if new_range.start >= old_range.end {
          // new deletion after old range, continue to next old range
          continue;
        }
      } else {
        // all remaining new_ranges are new deletions
        diff_ds.insert(
          ID::new(*client_id, new_range.start),
          new_range.end - new_range.start,
        );
      }
    }
  }

  diff_ds
}

#[inline]
fn intersects(x: &Range<u32>, y: &Range<u32>) -> bool {
  x.start < y.end && y.start < x.end
}

#[derive(Debug)]
enum Action {
  InitUser(UserDescription, MapRef),
  UserOverridden(UserDescription),
}

#[cfg(test)]
mod test {
  use crate::core::origin::{CollabClient, CollabOrigin};
  use crate::preclude::{Doc, PermanentUserData};
  use std::collections::HashSet;
  use yrs::updates::decoder::Decode;
  use yrs::{ReadTxn, Snapshot, StateVector, Text, Transact, Update};

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
}
