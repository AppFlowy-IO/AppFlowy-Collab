use crate::fields::FieldChangeSender;
use tokio::sync::broadcast;

use crate::rows::RowChangeSender;
use crate::views::ViewChangeSender;

#[derive(Clone)]
pub struct DatabaseNotify {
  pub view_change_tx: ViewChangeSender,
  pub row_change_tx: RowChangeSender,
  pub field_change_tx: FieldChangeSender,
}

impl Default for DatabaseNotify {
  fn default() -> Self {
    let (view_change_tx, _) = broadcast::channel(100);
    let (row_change_tx, _) = broadcast::channel(100);
    let (field_change_tx, _) = broadcast::channel(100);
    Self {
      view_change_tx,
      row_change_tx,
      field_change_tx,
    }
  }
}
