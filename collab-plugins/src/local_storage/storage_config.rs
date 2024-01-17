#[derive(Clone)]
pub struct CollabPersistenceConfig {
  /// Enable snapshot. Default is [false].
  pub enable_snapshot: bool,
  /// Generate a snapshot every N updates
  /// Default is 100. The value must be greater than 0.
  pub snapshot_per_update: u32,
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
}

impl Default for CollabPersistenceConfig {
  fn default() -> Self {
    Self {
      enable_snapshot: true,
      snapshot_per_update: 100,
    }
  }
}
