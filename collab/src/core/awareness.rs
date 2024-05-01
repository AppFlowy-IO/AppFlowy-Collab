use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::core::origin::CollabOrigin;
use crate::error::CollabError;
use thiserror::Error;
use yrs::block::ClientID;
use yrs::updates::decoder::{Decode, Decoder};
use yrs::updates::encoder::{Encode, Encoder};
use yrs::{Doc, Observer, Subscription};

/// The Awareness class implements a simple shared state protocol that can be used for non-persistent
/// data like awareness information (cursor, username, status, ..). Each client can update its own
/// local state and listen to state changes of remote clients.
///
/// Each client is identified by a unique client id (something we borrow from `doc.clientID`).
/// A client can override its own state by propagating a message with an increasing timestamp
/// (`clock`). If such a message is received, it is applied if the known state of that client is
/// older than the new state (`clock < new_clock`). If a client thinks that a remote client is
/// offline, it may propagate a message with `{ clock, state: null, client }`. If such a message is
/// received, and the known clock of that client equals the received clock, it will clean the state.
///
/// Before a client disconnects, it should propagate a `null` state with an updated clock.
pub struct Awareness {
  doc: Doc,
  states: HashMap<ClientID, Value>,
  meta: HashMap<ClientID, MetaClientState>,
  origin: CollabOrigin,
  #[allow(clippy::type_complexity)]
  on_update: Option<Observer<Arc<dyn Fn(&Awareness, &Event, &CollabOrigin) + 'static>>>,
}

unsafe impl Send for Awareness {}
unsafe impl Sync for Awareness {}

impl Awareness {
  /// Creates a new instance of [Awareness] struct, which operates over a given document.
  /// Awareness instance has full ownership of that document. If necessary it can be accessed
  /// using either [Awareness::doc] or [Awareness::doc_mut] methods.
  pub fn new(doc: Doc, origin: CollabOrigin) -> Self {
    Awareness {
      doc,
      on_update: None,
      states: HashMap::new(),
      meta: HashMap::new(),
      origin,
    }
  }

  pub fn with_observer<F>(doc: Doc, origin: CollabOrigin, f: F) -> Self
  where
    F: Fn(&Awareness, &Event, &CollabOrigin) + 'static,
  {
    let mut awareness = Awareness::new(doc, origin);
    awareness.on_update(f);
    awareness
  }

  /// Returns a channel receiver for an incoming awareness events. This channel can be cloned.
  pub fn on_update<F>(&mut self, f: F) -> AwarenessUpdateSubscription
  where
    F: Fn(&Awareness, &Event, &CollabOrigin) + 'static,
  {
    let eh = self.on_update.get_or_insert_with(Observer::default);
    eh.subscribe(f)
  }

  pub fn doc(&self) -> &Doc {
    &self.doc
  }

  pub fn doc_mut(&mut self) -> &mut Doc {
    &mut self.doc
  }

  /// Returns a globally unique client ID of an underlying [Doc].
  pub fn client_id(&self) -> ClientID {
    self.doc.client_id()
  }

  /// Returns a state map of all of the clients tracked by current [Awareness] instance. Those
  /// states are identified by their corresponding [ClientID]s. The associated state is
  /// represented and replicated to other clients as a JSON string.
  pub fn get_states(&self) -> &HashMap<ClientID, Value> {
    &self.states
  }

  /// Returns a JSON string state representation of a current [Awareness] instance.
  pub fn get_local_state(&self) -> Option<&Value> {
    self.states.get(&self.doc.client_id())
  }

  /// Sets the local state for the current [Awareness] instance to a specified JSON string.
  ///
  /// This method updates the state associated with the client ID obtained from `self.doc.client_id()`.
  /// The updated state is then replicated to other clients as part of the [AwarenessUpdate]. If an
  /// observer was set using [Awareness::with_observer], this method triggers an event to notify about
  /// the state change.
  ///
  /// The method checks whether the state update corresponds to a new or existing client. Depending on
  /// this check, it constructs an event indicating the nature of the update (new or updated client)
  /// and invokes all registered callbacks with this event.
  ///
  /// # Arguments
  /// * `json` - A string or a type that can be converted into a String, representing the new state
  ///   to be set for the current client ID.
  pub fn set_local_state<S: Into<Value>>(&mut self, json: S) {
    let client_id = self.doc.client_id();
    self.update_meta(client_id);

    let is_new_client = !self.states.contains_key(&client_id);
    self.states.insert(client_id, json.into());

    // Check if there's an update handler, and if so, create the appropriate event.
    if let Some(eh) = self.on_update.as_ref() {
      // The event varies if it's a new client or not.
      let e = if is_new_client {
        Event::new(vec![client_id], vec![], vec![])
      } else {
        Event::new(vec![], vec![client_id], vec![])
      };

      // Invoke callbacks with the event.
      for cb in eh.callbacks() {
        cb(self, &e, &self.origin);
      }
    }
  }

  /// Removes the state associated with a specified client ID from the current [Awareness] instance,
  /// effectively marking the client as disconnected.
  ///
  /// This method also triggers an [AwarenessUpdate] if the state for the provided client ID existed
  /// and was successfully removed. In such cases, if an observer was previously set using
  /// [Awareness::with_observer], this removal prompts an event to be emitted. The event signifies
  /// the disconnection of the client and notifies all registered callbacks.
  pub fn remove_state(&mut self, client_id: ClientID) {
    let prev_state = self.states.remove(&client_id);
    self.update_meta(client_id);

    if prev_state.is_some() {
      if let Some(eh) = self.on_update.as_ref() {
        let e = Event::new(Vec::default(), Vec::default(), vec![client_id]);
        for cb in eh.callbacks() {
          cb(self, &e, &self.origin);
        }
      }
    }
  }

  /// Clears out a state of a current client (see: [Awareness::client_id]),
  /// effectively marking it as disconnected.
  pub fn clean_local_state(&mut self) {
    let client_id = self.doc.client_id();
    self.remove_state(client_id);
  }

  fn update_meta(&mut self, client_id: ClientID) {
    let now = chrono::Utc::now().timestamp();
    match self.meta.entry(client_id) {
      Entry::Occupied(mut e) => {
        let clock = e.get().clock + 1;
        let meta = MetaClientState::new(clock, now);
        e.insert(meta);
      },
      Entry::Vacant(e) => {
        e.insert(MetaClientState::new(1, now));
      },
    }
  }

  /// Returns a serializable update object which is representation of a current Awareness state.
  pub fn update(&self) -> Result<AwarenessUpdate, Error> {
    let clients = self.states.keys().cloned();
    self.update_with_clients(clients)
  }

  /// Updates client states and generates an awareness update.
  ///
  /// Iterates over given client IDs to collect their states and clocks. If a client's metadata
  /// is missing, returns an `Error::ClientNotFound`. Otherwise, compiles all states into an
  /// `AwarenessUpdate`.
  ///
  /// # Arguments
  /// * `clients` - An iterable of client IDs to update.
  ///
  /// # Returns
  /// * `Ok(AwarenessUpdate)` containing the states of specified clients if all metadata is found.
  /// * `Err(Error::ClientNotFound)` with the ID of the missing client if any metadata is missing.
  ///
  pub fn update_with_clients<I: IntoIterator<Item = ClientID>>(
    &self,
    clients: I,
  ) -> Result<AwarenessUpdate, Error> {
    let mut res = HashMap::new();
    for client_id in clients {
      let clock = self
        .meta
        .get(&client_id)
        .ok_or(Error::ClientNotFound(client_id))?
        .clock;

      let json = self.states.get(&client_id).cloned().unwrap_or(Value::Null);

      res.insert(client_id, AwarenessUpdateEntry { clock, json });
    }
    Ok(AwarenessUpdate { clients: res })
  }

  /// Applies an update (incoming from remote channel or generated using [Awareness::update] /
  /// [Awareness::update_with_clients] methods) and modifies a state of a current instance.
  ///
  /// If current instance has an observer channel (see: [Awareness::with_observer]), applied
  /// changes will also be emitted as events.
  pub fn apply_update(
    &mut self,
    update: AwarenessUpdate,
    origin: &CollabOrigin,
  ) -> Result<(), Error> {
    let now = chrono::Utc::now().timestamp();

    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut removed = Vec::new();

    for (client_id, update_entry) in update.clients {
      let mut clock = update_entry.clock;
      let is_null = update_entry.json == Value::Null;
      match self.meta.entry(client_id) {
        Entry::Occupied(mut entry) => {
          let prev = entry.get();
          let is_removed = prev.clock == clock && is_null && self.states.contains_key(&client_id);
          let is_new = prev.clock < clock;
          if is_new || is_removed {
            if is_null {
              // never let a remote client remove this local state
              if client_id == self.doc.client_id() && self.states.get(&client_id).is_some() {
                // remote client removed the local state. Do not remote state. Broadcast a message indicating
                // that this client still exists by increasing the clock
                clock += 1;
              } else {
                self.states.remove(&client_id);
                if self.on_update.is_some() {
                  removed.push(client_id);
                }
              }
            } else {
              match self.states.entry(client_id) {
                Entry::Occupied(mut e) => {
                  if self.on_update.is_some() {
                    updated.push(client_id);
                  }
                  e.insert(update_entry.json);
                },
                Entry::Vacant(e) => {
                  e.insert(update_entry.json);
                  if self.on_update.is_some() {
                    updated.push(client_id);
                  }
                },
              }
            }
            entry.insert(MetaClientState::new(clock, now));
            // true
          } else {
            // false
          }
        },
        Entry::Vacant(e) => {
          e.insert(MetaClientState::new(clock, now));
          self.states.insert(client_id, update_entry.json);
          if self.on_update.is_some() {
            added.push(client_id);
          }
          // true
        },
      };
    }

    if let Some(eh) = self.on_update.as_ref() {
      if !added.is_empty() || !updated.is_empty() || !removed.is_empty() {
        let e = Event::new(added, updated, removed);
        for cb in eh.callbacks() {
          cb(self, &e, origin);
        }
      }
    }

    Ok(())
  }
}

impl std::fmt::Debug for Awareness {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Awareness")
      .field("state", &self.states)
      .field("meta", &self.meta)
      .field("doc", &self.doc)
      .finish()
  }
}

/// Whenever a new callback is being registered, a [Subscription] is made. Whenever this
/// subscription a registered callback is cancelled and will not be called any more.
pub type AwarenessUpdateSubscription = Subscription;

/// A structure that represents an encodable state of an [Awareness] struct.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AwarenessUpdate {
  pub(crate) clients: HashMap<ClientID, AwarenessUpdateEntry>,
}

impl AwarenessUpdate {
  pub fn clients(&self) -> &HashMap<ClientID, AwarenessUpdateEntry> {
    &self.clients
  }
}

impl Display for AwarenessUpdate {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    for client in self.clients.iter() {
      write!(f, "{}", client.1)?;
    }
    Ok(())
  }
}

impl Encode for AwarenessUpdate {
  fn encode<E: Encoder>(&self, encoder: &mut E) {
    encoder.write_var(self.clients.len());
    for (&client_id, e) in self.clients.iter() {
      encoder.write_var(client_id);
      encoder.write_var(e.clock);
      encoder.write_string(&e.json.to_string());
    }
  }
}

impl Decode for AwarenessUpdate {
  fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, yrs::encoding::read::Error> {
    let len: usize = decoder.read_var()?;
    let mut clients = HashMap::with_capacity(len);
    for _ in 0..len {
      let client_id: ClientID = decoder.read_var()?;
      let clock: u32 = decoder.read_var()?;
      let json = serde_json::from_str(decoder.read_string()?)?;
      clients.insert(client_id, AwarenessUpdateEntry { clock, json });
    }

    Ok(AwarenessUpdate { clients })
  }
}

/// A single client entry of an [AwarenessUpdate]. It consists of logical clock and JSON client
/// state represented as a string.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct AwarenessUpdateEntry {
  pub(crate) clock: u32,
  pub(crate) json: Value,
}

impl AwarenessUpdateEntry {
  pub fn json(&self) -> &Value {
    &self.json
  }
}

impl Display for AwarenessUpdateEntry {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "AwarenessUpdateEntry {{ clock: {}, json: {} }}",
      self.clock, self.json
    )
  }
}

/// Errors generated by an [Awareness] struct methods.
#[derive(Error, Debug)]
pub enum Error {
  /// Client ID was not found in [Awareness] metadata.
  #[error("client ID `{0}` not found")]
  ClientNotFound(ClientID),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetaClientState {
  clock: u32,
  last_updated: i64,
}

impl MetaClientState {
  fn new(clock: u32, last_updated: i64) -> Self {
    MetaClientState {
      clock,
      last_updated,
    }
  }
}

/// Event type emitted by an [Awareness] struct.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Event {
  added: Vec<ClientID>,
  updated: Vec<ClientID>,
  removed: Vec<ClientID>,
}

impl Event {
  pub fn new(added: Vec<ClientID>, updated: Vec<ClientID>, removed: Vec<ClientID>) -> Self {
    Event {
      added,
      updated,
      removed,
    }
  }

  /// Collection of new clients that have been added to an [Awareness] struct, that was not known
  /// before. Actual client state can be accessed via `awareness.clients().get(client_id)`.
  pub fn added(&self) -> &[ClientID] {
    &self.added
  }

  /// Collection of new clients that have been updated within an [Awareness] struct since the last
  /// update. Actual client state can be accessed via `awareness.clients().get(client_id)`.
  pub fn updated(&self) -> &[ClientID] {
    &self.updated
  }

  /// Collection of new clients that have been removed from [Awareness] struct since the last
  /// update.
  pub fn removed(&self) -> &[ClientID] {
    &self.removed
  }
}

#[inline]
pub fn gen_awareness_update_message(
  awareness: &Awareness,
  event: &Event,
) -> Result<AwarenessUpdate, CollabError> {
  let added = event.added();
  let updated = event.updated();
  let removed = event.removed();
  let mut changed = Vec::with_capacity(added.len() + updated.len() + removed.len());
  changed.extend_from_slice(added);
  changed.extend_from_slice(updated);
  changed.extend_from_slice(removed);
  let update = awareness.update_with_clients(changed)?;
  Ok(update)
}
