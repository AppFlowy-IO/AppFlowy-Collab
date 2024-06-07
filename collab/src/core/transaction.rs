use std::ops::{Deref, DerefMut};

use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transact, Transaction as Tx, TransactionMut as TxMut};

use crate::core::collab::CollabInner;
use crate::core::origin::CollabOrigin;
use crate::entity::EncodedCollab;

/// A wrapper around the Yrs readonly transaction, which also holds the lock to the collab state.
/// This way it can be obtained from the collab object using async transaction acquisition.
pub struct Transaction<'a> {
  lock: RwLockReadGuard<'a, CollabInner>,
  inner: Tx<'a>,
}

impl<'a> Transaction<'a> {
  pub(crate) fn new(lock: RwLockReadGuard<'a, CollabInner>) -> Self {
    let inner = lock.doc.transact();
    Self { lock, inner }
  }
}

impl<'a> Deref for Transaction<'a> {
  type Target = Tx<'a>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

/// A wrapper around the Yrs read-write transaction, which also holds the lock to the collab state.
/// This way it can be obtained from the collab object using async transaction acquisition.
pub struct TransactionMut<'a> {
  lock: RwLockWriteGuard<'a, CollabInner>,
  inner: TxMut<'a>,
}

impl<'a> TransactionMut<'a> {
  pub(crate) fn new(lock: RwLockWriteGuard<'a, CollabInner>, origin: CollabOrigin) -> Self {
    let inner = lock.doc.transact_mut_with(origin);
    Self { lock, inner }
  }
}

impl<'a> Deref for TransactionMut<'a> {
  type Target = TxMut<'a>;

  #[inline]
  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl<'a> DerefMut for TransactionMut<'a> {
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

impl<'a> DocTransactionExtension for Tx<'a> {}
impl<'a> DocTransactionExtension for TxMut<'a> {}
