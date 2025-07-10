use crate::error::CollabError;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use uuid::{NoContext, Timestamp, Uuid};
use yrs::encoding::serde::{from_any, to_any};
use yrs::types::array::ArrayIter;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Array, ArrayRef, Out, ReadTxn, Snapshot, TransactionMut};

pub type RevisionId = Uuid;

/// Revision is a record of a collab state at a specific point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
  id: RevisionId,
  name: Option<String>,
  snapshot_state: Vec<u8>,
}

impl Revision {
  /// Creates a new [Revision] with the given name and snapshot state.
  pub fn new(name: Option<String>, snapshot_state: &Snapshot) -> Self {
    Self {
      id: RevisionId::new_v7(Timestamp::now(NoContext)),
      name,
      snapshot_state: snapshot_state.encode_v1(),
    }
  }

  /// Globally unique identifier for the revision.
  pub fn id(&self) -> &RevisionId {
    &self.id
  }

  /// Returns the timestamp when the revision was created.
  pub fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
    let timestamp = self.id.get_timestamp()?;
    let (seconds, nanos) = timestamp.to_unix();
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds as i64, nanos)
  }

  /// Returns the name of the revision, if it exists.
  pub fn name(&self) -> Option<&str> {
    self.name.as_deref()
  }

  /// Deserializes revision's snapshot state into a yrs [Snapshot].
  pub fn snapshot(&self) -> Result<Snapshot, CollabError> {
    Ok(Snapshot::decode_v1(&self.snapshot_state)?)
  }
}

pub struct Revisions {
  revisions: ArrayRef,
}

impl Revisions {
  pub fn new(revisions: ArrayRef) -> Self {
    Self { revisions }
  }

  pub fn create_revision(
    &self,
    txn: &mut TransactionMut,
    name: Option<String>,
  ) -> Result<RevisionId, CollabError> {
    if !txn.doc().skip_gc() {
      return Err(CollabError::Internal(anyhow!(
        "revisions cannot be created when garbage collection is enabled"
      )));
    }

    let snapshot = txn.snapshot();
    let revision = Revision::new(name, &snapshot);
    let encoded_revision = to_any(&revision).map_err(|err| CollabError::Internal(err.into()))?;

    self.revisions.push_back(txn, encoded_revision);

    Ok(revision.id)
  }

  pub fn get<T: ReadTxn>(
    &self,
    txn: &T,
    revision_id: &RevisionId,
  ) -> Result<Revision, CollabError> {
    let i = self.iter(txn);
    for revision in i {
      let revision = revision?;
      if &revision.id == revision_id {
        return Ok(revision);
      }
    }

    Err(CollabError::NoRequiredData(format!(
      "Revision '{revision_id}' not found"
    )))
  }

  pub fn remove_where<F>(
    &self,
    txn: &mut TransactionMut,
    predicate: F,
  ) -> Result<usize, CollabError>
  where
    F: Fn(&Revision) -> bool,
  {
    if !txn.doc().skip_gc() {
      return Err(CollabError::Internal(anyhow!(
        "revisions cannot be drained when garbage collection is enabled"
      )));
    }

    let mut oldest: Option<Revision> = None;
    let mut revisions_to_remove = Vec::new();
    for (index, revision) in self.iter(txn).enumerate() {
      if let Ok(revision) = revision {
        if predicate(&revision) {
          // if the predicate matches, we mark this revision for removal.
          revisions_to_remove.push(index as u32);
        } else {
          // if the predicate does not match, we keep track of the oldest revision for future gc
          match &oldest {
            None => oldest = Some(revision),
            Some(oldest_revision) => {
              if revision.created_at() < oldest_revision.created_at() {
                oldest = Some(revision);
              }
            },
          }
        }
      }
    }

    // remove revisions that matched the predicate
    // use reverse order (last-to-first) to avoid shifting indices
    let result = revisions_to_remove.len();
    for index in revisions_to_remove.into_iter().rev() {
      self.revisions.remove(txn, index);
    }

    // garbage collect revisions
    match oldest {
      Some(oldest_revision) => {
        // if we have the oldest revision, we can safely gc everything up to that revision
        let snapshot = oldest_revision.snapshot()?;
        txn.gc(Some(&snapshot.delete_set));
      },
      None => txn.gc(None), // there are no revisions left, we can safely gc everything
    }

    Ok(result)
  }

  pub fn iter<'a, T: ReadTxn>(&self, txn: &'a T) -> RevisionsIter<'a, T> {
    let iter = self.revisions.iter(txn);
    RevisionsIter { iter }
  }
}

pub struct RevisionsIter<'a, T: ReadTxn> {
  iter: ArrayIter<&'a T, T>,
}

impl<T: ReadTxn> Iterator for RevisionsIter<'_, T> {
  type Item = Result<Revision, CollabError>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.iter.next()? {
      Out::Any(revision) => {
        let revision =
          from_any::<Revision>(&revision).map_err(|err| CollabError::Internal(err.into()));
        Some(revision)
      },
      _ => Some(Err(CollabError::NoRequiredData(
        "Cannot decode revision".to_string(),
      ))),
    }
  }
}
