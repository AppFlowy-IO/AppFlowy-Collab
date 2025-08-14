use collab::preclude::{Any, Map, MapRef, Out, ReadTxn, TransactionMut};

pub struct RevisionMapping {
  container: MapRef,
}

impl RevisionMapping {
  /// The maximum number of jumps to prevent infinite loops in the revision map.
  const REVISION_MAP_JUMP_LIMIT: usize = 1000;

  pub fn new(container: MapRef) -> Self {
    Self { container }
  }

  pub fn contains_key<T: ReadTxn>(&self, txn: &T, key: &str) -> bool {
    self.container.contains_key(txn, key)
  }

  pub fn replace_view(&self, txn: &mut TransactionMut, old_view_id: &str, new_view_id: &str) {
    if self.container.contains_key(txn, new_view_id) {
      // new view id should not already exist in the revision map, otherwise it could create a cycle
      panic!(
        "new view_id {} already exists in the revision map",
        new_view_id
      );
    }

    self.container.insert(txn, old_view_id, new_view_id);
  }

  pub fn mappings(&self, txn: &impl ReadTxn, view_id: String) -> (String, Vec<String>) {
    let mut buf = Vec::new();
    let last_view_id = self.iter_mapping(txn, view_id, |view_id| {
      buf.push(view_id);
    });
    (last_view_id, buf)
  }

  pub fn map<T: ReadTxn>(&self, txn: &T, view_id: String) -> String {
    self.iter_mapping(txn, view_id, |_| {})
  }

  fn iter_mapping<T, F>(&self, txn: &T, view_id: String, mut f: F) -> String
  where
    T: ReadTxn,
    F: FnMut(String),
  {
    let mut current_view_id = view_id;
    let mut i = Self::REVISION_MAP_JUMP_LIMIT;
    while i > 0 {
      if let Some(Out::Any(Any::String(next_view_id))) = self.container.get(txn, &current_view_id) {
        let next_view_id = next_view_id.to_string();
        let old_view_id = std::mem::replace(&mut current_view_id, next_view_id);
        f(old_view_id);
        i -= 1;
      } else {
        break;
      }
    }
    if i == 0 {
      panic!(
        "Infinite loop detected in revision map for view_id: {}",
        current_view_id
      );
    }
    current_view_id
  }
}
