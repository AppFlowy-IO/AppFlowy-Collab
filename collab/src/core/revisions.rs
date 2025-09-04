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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
  pub id: RevisionId,
  pub name: Option<String>,
  pub snapshot_state: Vec<u8>,
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

  /// Returns the timestamp when the revision was created.
  pub fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
    let timestamp = self.id.get_timestamp()?;
    let (seconds, nanos) = timestamp.to_unix();
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds as i64, nanos)
  }

  /// Deserializes revision's snapshot state into a yrs [Snapshot].
  pub fn snapshot(&self) -> Result<Snapshot, CollabError> {
    Ok(Snapshot::decode_v1(&self.snapshot_state)?)
  }
}

/// It's a collaborative collection used to store document history revisions.
/// These revisions can be used to restore the document state at a specific version.
///
/// Revisions are not compatible with garbage collection, so they must be created with
/// garbage collection disabled.
#[derive(Debug, Clone)]
pub struct Revisions {
  revisions: ArrayRef,
}

impl Revisions {
  pub fn new(revisions: ArrayRef) -> Self {
    Self { revisions }
  }

  /// Creates a new revision with the given name, if provided.
  /// Returns the unique identifier of the created revision.
  pub fn create_revision(
    &self,
    txn: &mut TransactionMut,
    name: Option<String>,
  ) -> Result<RevisionId, CollabError> {
    ensure_gc_disabled(txn)?;

    let snapshot = txn.snapshot();
    let revision = Revision::new(name, &snapshot);
    let encoded_revision = to_any(&revision).map_err(|err| CollabError::Internal(err.into()))?;

    self.revisions.push_back(txn, encoded_revision);

    Ok(revision.id)
  }

  /// Returns a revision by its unique identifier.
  /// If the revision is not found, it returns an [CollabError::NoRequiredData] error.
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

  /// Removes all revisions matching the specified predicate.
  ///
  /// This method will also garbage collect the data stored inside the collab, that is no longer
  /// accessible but was required by the removed revisions for the sake of restoring past document
  /// state.
  pub fn remove_where<F>(
    &self,
    txn: &mut TransactionMut,
    mut predicate: F,
  ) -> Result<usize, CollabError>
  where
    F: FnMut(&Revision) -> bool,
  {
    ensure_gc_disabled(txn)?;

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

  /// Iterates over all active revisions in the collection.
  pub fn iter<'a, T: ReadTxn>(&self, txn: &'a T) -> RevisionsIter<'a, T> {
    let iter = self.revisions.iter(txn);
    RevisionsIter { iter }
  }

  pub fn as_vec(&self, txn: &impl ReadTxn) -> Result<Vec<Revision>, CollabError> {
    self.iter(txn).collect()
  }

  /// Performs garbage collection on the document, removing all data that is no longer
  /// accessible but might have been required by revisions in the past.
  pub fn gc(&self, txn: &mut TransactionMut) -> Result<(), CollabError> {
    ensure_gc_disabled(txn)?;

    // find the oldest revision to determine the cutoff point for garbage collection
    let mut oldest: Option<Revision> = None;
    for revision in self.iter(txn).flatten() {
      match &oldest {
        None => oldest = Some(revision),
        Some(oldest_revision) => {
          if revision.created_at() < oldest_revision.created_at() {
            oldest = Some(revision);
          }
        },
      }
    }

    let snapshot = match oldest {
      Some(oldest_revision) => Some(oldest_revision.snapshot()?),
      None => None,
    };

    txn.gc(snapshot.as_ref().map(|s| &s.delete_set));

    Ok(())
  }
}

/// Make sure that garbage collection on the document is disabled.
/// This is necessary because revisions are not compatible with garbage collection.
/// Garbage collection can still be performed manually with the respect to data required by revisions.
fn ensure_gc_disabled(txn: &TransactionMut) -> Result<(), CollabError> {
  if !txn.doc().skip_gc() {
    return Err(CollabError::Internal(anyhow!(
      "revisions cannot be created when garbage collection is enabled"
    )));
  }
  Ok(())
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
