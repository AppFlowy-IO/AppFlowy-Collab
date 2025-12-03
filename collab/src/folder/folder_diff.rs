use std::collections::HashSet;
use std::sync::Arc;

use crate::core::origin::CollabOrigin;
use crate::entity::EncodedCollab;
use crate::error::CollabError;
use crate::preclude::updates::decoder::Decode;
use crate::preclude::{
  ClientID, DeepObservable, EntryChange, Event, MapExt, ReadTxn, Update, YrsValue,
};
use arc_swap::ArcSwapOption;

use super::Folder;
use super::view::FOLDER_VIEW_ID;

impl Folder {
  pub fn calculate_view_changes(
    &self,
    encoded_collab: EncodedCollab,
    client_id: ClientID,
  ) -> Result<Vec<FolderViewChange>, CollabError> {
    //TODO: this entire method should be reimplemented into standard diff comparison
    let changes = Arc::new(ArcSwapOption::default());
    let this_txn = self.collab.transact();
    let workspace_id = self
      .body
      .get_workspace_id(&this_txn)
      .ok_or_else(|| CollabError::FolderMissingRequiredData("workspace id".to_string()))?;

    let mut other = Folder::from_collab_doc_state(
      CollabOrigin::Empty,
      encoded_collab.into(),
      &workspace_id,
      client_id,
    )?
    .folder;
    let cloned_container = other.body.views.container.clone();
    let sub = {
      let changes = changes.clone();
      cloned_container.observe_deep(move |txn, events| {
        let mut acc = HashSet::new();
        for event in events.iter() {
          if let Event::Map(event) = event {
            for c in event.keys(txn).values() {
              match c {
                EntryChange::Inserted(v) => {
                  if let YrsValue::YMap(map_ref) = v {
                    if let Some(view_id) = map_ref.get_with_txn(txn, FOLDER_VIEW_ID) {
                      acc.insert(FolderViewChange::Inserted { view_id });
                    }
                  }
                },
                EntryChange::Updated(_, _) => {
                  if let Some(view_id) = event.target().get_with_txn(txn, FOLDER_VIEW_ID) {
                    acc.insert(FolderViewChange::Updated { view_id });
                  }
                },
                EntryChange::Removed(v) => {
                  if let YrsValue::YMap(_map_ref) = v {
                    let deleted_view_ids = event
                      .keys(txn)
                      .iter()
                      .map(|(k, _)| (**k).to_owned())
                      .collect::<Vec<String>>();
                    acc.insert(FolderViewChange::Deleted {
                      view_ids: deleted_view_ids,
                    });
                  }
                },
              }
            }
          }
        }
        changes.store(Some(Arc::new(Vec::from_iter(acc))));
      })
    };
    {
      let mut other_txn = other.collab.transact_mut();
      let sv = other_txn.state_vector();
      let data = this_txn.encode_state_as_update_v1(&sv);
      let update = Update::decode_v1(&data).map_err(|err| CollabError::Internal(err.into()))?;

      other_txn
        .apply_update(update)
        .map_err(CollabError::UpdateFailed)?;
    }
    drop(sub);
    drop(other);

    match changes.swap(None) {
      None => Ok(vec![]),
      Some(changes) => {
        let result = Arc::into_inner(changes).unwrap();
        Ok(result)
      },
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FolderViewChange {
  Inserted { view_id: String },
  Updated { view_id: String },
  Deleted { view_ids: Vec<String> },
}
