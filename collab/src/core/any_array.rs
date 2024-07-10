use std::ops::{Deref, DerefMut};

use yrs::{Array, ArrayRef, ReadTxn, TransactionMut};

use crate::core::any_map::AnyMap;
use crate::core::array_wrapper::ArrayRefExtension;
use crate::preclude::{MapRefExtension, YrsValue};

/// A wrapper around an `ArrayRef` that allows to store `AnyMap` in it.
#[derive(Default, Debug)]
pub struct ArrayMap(pub Vec<AnyMap>);

impl ArrayMap {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn from_any_maps(items: Vec<AnyMap>) -> Self {
    let mut this = Self::new();
    for item in items {
      this.push(item);
    }
    this
  }

  /// Constructs an instance of the current struct from the provided `array_ref`.
  ///
  /// Iterates through each value within the `array_ref`. If a value is found to be
  /// of type `Value::YMap`, it constructs an `AnyMap` from the value and appends
  /// it to the resulting array. Unsupported types trigger a debug assertion.
  ///
  pub fn from_array_ref<R: ReadTxn>(txn: &R, array_ref: &ArrayRef) -> Self {
    let mut any_array = Self::new();
    for value in array_ref.iter(txn) {
      match value {
        YrsValue::YMap(map_ref) => {
          any_array.push(AnyMap::from((txn, &map_ref)));
        },
        _ => debug_assert!(false, "Unsupported type"),
      }
    }
    any_array
  }

  /// Sets the provided `array_ref` with the contents of the current struct.
  ///
  /// This function first clears the given `array_ref`, ensuring that it's empty.
  /// It then iterates through each value in the current struct (represented by `self.0`)
  /// and inserts a new map into the `array_ref`. The `value` is then used to fill this
  /// new map.
  pub fn set_array_ref(self, txn: &mut TransactionMut, array_ref: ArrayRef) {
    array_ref.clear(txn);
    for value in self.0 {
      let map_ref = array_ref.insert_map_with_txn(txn, None);
      value.fill_map_ref(txn, &map_ref);
    }
  }
}

impl Deref for ArrayMap {
  type Target = Vec<AnyMap>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for ArrayMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

/// A utility struct that provides functionality for performing updates on an array of maps.
///
/// This struct encapsulates the transaction and reference to an array to provide methods
/// for inserting, updating, and deleting `AnyMap` entries.
pub struct ArrayMapUpdate<'a, 'b> {
  array_ref: ArrayRef,
  txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ArrayMapUpdate<'a, 'b> {
  /// Creates a new `ArrayMapUpdate` with a given transaction and array reference.
  ///
  /// # Arguments
  ///
  /// * `txn`: A mutable reference to the current transaction.
  /// * `array_ref`: A reference to the array to be updated.
  pub fn new(txn: &'a mut TransactionMut<'b>, array_ref: ArrayRef) -> Self {
    Self { txn, array_ref }
  }

  /// Inserts an `AnyMap` into the array at a specific index.
  ///
  /// # Arguments
  ///
  /// * `any_map`: The `AnyMap` object to be inserted.
  /// * `index`: The position where the map should be inserted.
  pub fn insert(self, any_map: AnyMap, index: u32) -> Self {
    let map_ref = self
      .array_ref
      .insert_map_at_index_with_txn(self.txn, index, None);
    any_map.fill_map_ref(self.txn, &map_ref);
    self
  }

  pub fn push(self, any_map: AnyMap) -> Self {
    let map_ref = self.array_ref.insert_map_with_txn(self.txn, None);
    any_map.fill_map_ref(self.txn, &map_ref);
    self
  }

  /// Removes an `AnyMap` from the array by its ID.
  ///
  /// # Arguments
  ///
  /// * `id`: The ID of the `AnyMap` to be removed.
  pub fn remove(self, id: &str) -> Self {
    if let Some(pos) = self.index_of(id) {
      self.array_ref.remove(self.txn, pos);
    }
    self
  }

  pub fn clear(self) -> Self {
    let len = self.array_ref.len(self.txn);
    self.array_ref.remove_range(self.txn, 0, len);
    self
  }

  /// Updates an `AnyMap` by its ID, using the provided function.
  ///
  /// # Arguments
  ///
  /// * `id`: The ID of the `AnyMap` to be updated.
  /// * `f`: A function that takes an `AnyMap` and returns an updated `AnyMap`.
  pub fn update<F>(self, id: &str, f: F) -> Self
  where
    F: FnOnce(AnyMap) -> AnyMap,
  {
    if let Some(pos) = self.index_of(id) {
      if let YrsValue::YMap(map_ref) = self.array_ref.get(self.txn, pos).unwrap() {
        let any_map = AnyMap::from_map_ref(self.txn, &map_ref);
        f(any_map).fill_map_ref(self.txn, &map_ref);
      }
    }

    self
  }

  /// Checks if an `AnyMap` with a specific ID exists in the array.
  pub fn contains(&self, id: &str) -> bool {
    self.index_of(id).is_some()
  }

  /// Moves an `AnyMap` from one position to another in the array.
  pub fn move_to(self, from_id: &str, to_id: &str) {
    let from_pos = self.index_of(from_id);
    let to_pos = self.index_of(to_id);

    if let (Some(from), Some(to)) = (from_pos, to_pos) {
      self.array_ref.move_to(self.txn, from, to)
    }
  }

  /// Returns the index of an `AnyMap` with a specific ID.
  ///
  /// # Arguments
  ///
  /// * `id`: The ID of the `AnyMap`.
  ///
  /// # Returns
  ///
  /// Returns an `Option` containing the index if found, otherwise `None`.
  fn index_of(&self, id: &str) -> Option<u32> {
    self
      .array_ref
      .iter(self.txn)
      .position(|v| {
        if let YrsValue::YMap(map_ref) = v {
          if let Some(target_id) = map_ref.get_str_with_txn(self.txn, "id") {
            return target_id == id;
          }
        }
        false
      })
      .map(|v| v as u32)
  }
}
