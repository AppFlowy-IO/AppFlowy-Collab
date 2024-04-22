use std::thread::sleep;
use std::time::Duration;
use tracing::instrument;

use crate::core::collab_plugin::EncodedCollab;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Transaction, TransactionMut};

use crate::core::origin::CollabOrigin;
use crate::error::CollabError;

/// TransactionRetry is a wrapper of Transaction and TransactionMut.
/// It will retry to get a transaction if fail to require the transaction.
/// The default timeout is `2` seconds and the default retry interval is `50` milliseconds.
/// Most of the time, it will get the transaction in the first try.
pub struct TransactionRetry<'a> {
  timeout: Duration,
  doc: &'a Doc,
  timer: Timer,
  retry_interval: Duration,
  object_id: &'a str,
}

impl<'a> TransactionRetry<'a> {
  pub fn new(doc: &'a Doc, object_id: &'a str) -> Self {
    Self {
      timeout: Duration::from_secs(2),
      retry_interval: Duration::from_millis(500),
      doc,
      timer: Timer::start(),
      object_id,
    }
  }

  #[instrument(level = "trace", skip_all)]
  pub fn get_read_txn(&mut self) -> Transaction<'a> {
    while self.timer.elapsed() < self.timeout {
      match self.doc.try_transact() {
        Ok(txn) => {
          return txn;
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire read txn timeout: {}", self.object_id);
    self.doc.transact()
  }

  #[instrument(level = "trace", skip_all)]
  pub fn try_get_write_txn(&mut self) -> Result<TransactionMut<'a>, CollabError> {
    while self.timer.elapsed() < self.timeout {
      match self.doc.try_transact_mut() {
        Ok(txn) => {
          return Ok(txn);
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire write txn timeout: {}", self.object_id);
    Err(CollabError::AcquiredWriteTxnFail)
  }

  #[instrument(level = "trace", skip_all)]
  pub fn get_write_txn_with(&mut self, origin: CollabOrigin) -> TransactionMut<'a> {
    while self.timer.elapsed() < self.timeout {
      match self.doc.try_transact_mut_with(origin.clone()) {
        Ok(txn) => {
          return txn;
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire write txn timeout: {}", self.object_id);
    self.doc.transact_mut_with(origin)
  }

  #[instrument(level = "trace", skip_all)]
  pub fn try_get_write_txn_with(
    &mut self,
    origin: CollabOrigin,
  ) -> Result<TransactionMut<'a>, CollabError> {
    while self.timer.elapsed() < self.timeout {
      match self.doc.try_transact_mut_with(origin.clone()) {
        Ok(txn) => {
          return Ok(txn);
        },
        Err(_e) => {
          sleep(self.retry_interval);
        },
      }
    }
    tracing::warn!("[Txn]: acquire write txn timeout: {}", self.object_id);
    Err(CollabError::AcquiredWriteTxnFail)
  }
}

pub trait DocTransactionExtension: Send + Sync {
  fn doc_transaction(&self) -> Transaction;
  fn doc_transaction_mut(&self) -> TransactionMut;

  fn get_encoded_collab_v1(&self) -> EncodedCollab {
    let txn = self.doc_transaction();
    EncodedCollab::new_v1(
      txn.state_vector().encode_v1(),
      txn.encode_state_as_update_v1(&StateVector::default()),
    )
  }

  fn get_encoded_collab_v2(&self) -> EncodedCollab {
    let txn = self.doc_transaction();
    EncodedCollab::new_v2(
      txn.state_vector().encode_v2(),
      txn.encode_state_as_update_v2(&StateVector::default()),
    )
  }
}

impl DocTransactionExtension for Doc {
  fn doc_transaction(&self) -> Transaction {
    self.transact()
  }
  fn doc_transaction_mut(&self) -> TransactionMut {
    self.transact_mut()
  }
}

if_native! {
  struct Timer {
    start: std::time::Instant,
  }

  impl Timer {
    fn start() -> Self {
      Self { start: std::time::Instant::now() }
    }

    fn elapsed(&self) -> Duration {
      self.start.elapsed()
    }
  }
}

if_wasm! {
  struct Timer {
    start: f64,
  }

  impl Timer {
    fn start() -> Self {
      Self { start: js_sys::Date::now() }
    }

    fn elapsed(&self) -> Duration {
      let now = js_sys::Date::now();
      let elapsed_ms = now - self.start;
      Duration::from_millis(elapsed_ms as u64)
    }
  }
}
