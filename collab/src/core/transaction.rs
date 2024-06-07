use std::ops::{Deref, DerefMut};

use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transact, Transaction, TransactionMut};

use crate::core::collab::CollabInner;
use crate::core::origin::CollabOrigin;
use crate::entity::EncodedCollab;

/// A wrapper around the Yrs readonly transaction, which also holds the lock to the collab state.
/// This way it can be obtained from the collab object using async transaction acquisition.
pub struct LockedTransaction<'a> {
  #[allow(dead_code)]
  lock: RwLockReadGuard<'a, CollabInner>,
  inner: Transaction<'a>,
}

impl<'a> LockedTransaction<'a> {
  pub(crate) fn new(lock: RwLockReadGuard<'a, CollabInner>) -> Self {
    let txn = lock.doc.transact();
    // transaction never outlives its document,
    // while RwLock lifetime is scoped to document container
    let inner = unsafe { std::mem::transmute(txn) };
    Self { lock, inner }
  }
}

impl<'a> Deref for LockedTransaction<'a> {
  type Target = Transaction<'a>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

/// A wrapper around the Yrs read-write transaction, which also holds the lock to the collab state.
/// This way it can be obtained from the collab object using async transaction acquisition.
pub struct LockedTransactionMut<'a> {
  #[allow(dead_code)]
  lock: RwLockWriteGuard<'a, CollabInner>,
  inner: TransactionMut<'a>,
}

impl<'a> LockedTransactionMut<'a> {
  pub(crate) fn new(lock: RwLockWriteGuard<'a, CollabInner>, origin: CollabOrigin) -> Self {
    let txn = lock.doc.transact_mut_with(origin);
    // transaction never outlives its document,
    // while RwLock lifetime is scoped to document container
    let inner = unsafe { std::mem::transmute(txn) };
    Self { lock, inner }
  }
}

impl<'a> Deref for LockedTransactionMut<'a> {
  type Target = TransactionMut<'a>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<'a> DerefMut for LockedTransactionMut<'a> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

pub trait DocTransactionExtension: ReadTxn {
  fn get_encoded_collab_v1(&self) -> EncodedCollab {
    EncodedCollab::new_v1(
      self.state_vector().encode_v1(),
      self.encode_state_as_update_v1(&StateVector::default()),
    )
  }

  fn get_encoded_collab_v2(&self) -> EncodedCollab {
    EncodedCollab::new_v2(
      self.state_vector().encode_v2(),
      self.encode_state_as_update_v2(&StateVector::default()),
    )
  }
}

impl<'a> DocTransactionExtension for Transaction<'a> {}
impl<'a> DocTransactionExtension for TransactionMut<'a> {}
