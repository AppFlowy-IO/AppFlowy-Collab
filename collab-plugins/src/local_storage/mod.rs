#[cfg(feature = "rocksdb_plugin")]
pub mod rocksdb;

#[derive(Clone)]
pub struct CollabPersistenceConfig {
  /// Enable snapshot. Default is [false].
  pub enable_snapshot: bool,
  /// Generate a snapshot every N updates
  /// Default is 20. The value must be greater than 0.
  pub snapshot_per_update: u32,

  /// Flush the document. Default is [false].
  /// After flush the document, all updates will be removed and the document state vector that
  /// contains all the updates will be reset.
  pub(crate) flush_doc: bool,
}

impl CollabPersistenceConfig {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn enable_snapshot(mut self, enable_snapshot: bool) -> Self {
    self.enable_snapshot = enable_snapshot;
    self
  }

  pub fn snapshot_per_update(mut self, snapshot_per_update: u32) -> Self {
    debug_assert!(snapshot_per_update > 0);
    self.snapshot_per_update = snapshot_per_update;
    self
  }

  pub fn flush_doc(mut self, flush_doc: bool) -> Self {
    self.flush_doc = flush_doc;
    self
  }
}

impl Default for CollabPersistenceConfig {
  fn default() -> Self {
    Self {
      enable_snapshot: true,
      snapshot_per_update: 100,
      flush_doc: false,
    }
  }
}
