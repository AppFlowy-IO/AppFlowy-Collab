use crate::folder::FAVORITES_V1;
use crate::{FavoriteId, Folder};
use collab::preclude::Array;

impl Folder {
  /// Retrieves historical favorite data from the key `FAVORITES_V1`.
  /// Note: `FAVORITES_V1` is deprecated. Use `FAVORITES_V2` for storing favorite data.
  ///
  /// Returns a `Vec<FavoriteId>` containing the historical favorite data.
  /// The vector will be empty if no historical favorite data exists.
  pub fn get_favorite_v1(&self) -> Vec<FavoriteId> {
    let txn = self.root.transact();
    let mut favorites = vec![];
    if let Some(favorite_array) = self.root.get_array_ref_with_txn(&txn, FAVORITES_V1) {
      for record in favorite_array.iter(&txn) {
        if let Ok(id) = FavoriteId::try_from(&record) {
          favorites.push(id);
        }
      }
    }
    favorites
  }
}
