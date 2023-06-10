use std::thread::sleep;
use std::time::{Duration, Instant};

use yrs::{Doc, Transact, Transaction, TransactionMut};

use crate::core::origin::CollabOrigin;
use crate::error::CollabError;

/// TransactionRetry is a wrapper of Transaction and TransactionMut.
/// It will retry to get a transaction if fail to require the transaction.
/// The default timeout is `2` seconds and the default retry interval is `50` milliseconds.
/// Most of the time, it will get the transaction in the first try.
pub struct TransactionRetry<'a> {
  timeout: Duration,
  doc: &'a Doc,
  start: Instant,
  retry_interval: Duration,
}

impl<'a> TransactionRetry<'a> {
  pub fn new(doc: &'a Doc) -> Self {
    Self {
      timeout: Duration::from_secs(2),
      retry_interval: Duration::from_millis(50),
      doc,
      start: Instant::now(),
    }
  }

  pub fn get_read_txn(&mut self) -> Transaction<'a> {
    while self.start.elapsed() < self.timeout {
      match self.doc.try_transact() {
        Ok(txn) => {
          return txn;
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire read txn timeout");
    self.doc.transact()
  }

  pub fn get_write_txn_with(&mut self, origin: CollabOrigin) -> TransactionMut<'a> {
    while self.start.elapsed() < self.timeout {
      match self.doc.try_transact_mut_with(origin.clone()) {
        Ok(txn) => {
          return txn;
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire write txn timeout");
    self.doc.transact_mut_with(origin)
  }

  pub fn try_get_write_txn_with(
    &mut self,
    origin: CollabOrigin,
  ) -> Result<TransactionMut<'a>, CollabError> {
    while self.start.elapsed() < self.timeout {
      match self.doc.try_transact_mut_with(origin.clone()) {
        Ok(txn) => {
          return Ok(txn);
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire write txn timeout");
    Err(CollabError::AcquiredWriteTxnFail)
  }
}
