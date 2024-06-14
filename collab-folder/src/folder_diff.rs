use crate::error::FolderError;
use crate::view::FOLDER_VIEW_ID;
use crate::Folder;
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::entity::EncodedCollab;
use collab::preclude::updates::decoder::Decode;
use collab::preclude::{DeepObservable, EntryChange, Event, ReadTxn, Update, YrsValue};
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

impl Folder {
  pub fn calculate_view_changes(
    &self,
    encoded_collab: EncodedCollab,
  ) -> Result<Vec<FolderViewChange>, FolderError> {
    let changes = Arc::new(Mutex::new(HashSet::new()));
    let workspace_id = self.try_get_workspace_id()?;

    let other = Folder::from_collab_doc_state(
      self.uid.clone(),
      CollabOrigin::Empty,
      DataSource::DocStateV1(encoded_collab.doc_state.to_vec()),
      &workspace_id,
      vec![],
    )?;
    let cloned_container = other.views.container.clone();
    let cloned_changes = changes.clone();
    let sub = cloned_container.observe_deep(move |txn, events| {
      let mut changes = cloned_changes.lock();
      for event in events.iter() {
        if let Event::Map(event) = event {
          for c in event.keys(txn).values() {
            match c {
              EntryChange::Inserted(v) => {
                if let YrsValue::YMap(map_ref) = v {
                  if let Some(view_id) = map_ref.get_str_with_txn(txn, FOLDER_VIEW_ID) {
                    changes.insert(FolderViewChange::Inserted { view_id });
                  }
                }
              },
              EntryChange::Updated(_k, v) => {
                println!("Updated: {}: {:?}", _k, v);
                if let Some(view_id) = event.target().get_str_with_txn(txn, FOLDER_VIEW_ID) {
                  changes.insert(FolderViewChange::Updated { view_id });
                }
              },
              EntryChange::Removed(v) => {
                if let YrsValue::YMap(_map_ref) = v {
                  let deleted_view_ids = event
                    .keys(txn)
                    .iter()
                    .map(|(k, _)| (**k).to_owned())
                    .collect::<Vec<String>>();
                  changes.insert(FolderViewChange::Deleted {
                    view_ids: deleted_view_ids,
                  });
                }
              },
            }
          }
        }
      }
    });
    let lock_guard = other.inner.lock();
    let sv = lock_guard.transact().state_vector();
    let data = self.inner.lock().transact().encode_state_as_update_v1(&sv);
    let update = Update::decode_v1(&data).map_err(|err| FolderError::Internal(err.into()))?;

    let mut txn = lock_guard.try_transaction_mut()?;
    txn.apply_update(update);
    drop(txn);
    drop(sub);

    let lock_guard = changes.lock();
    Ok(
      lock_guard
        .iter()
        .cloned()
        .collect::<Vec<FolderViewChange>>(),
    )
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FolderViewChange {
  Inserted { view_id: String },
  Updated { view_id: String },
  Deleted { view_ids: Vec<String> },
}
