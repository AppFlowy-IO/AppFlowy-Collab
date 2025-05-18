use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, Transaction, TransactionMut};

use crate::entity::EncodedCollab;

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

impl DocTransactionExtension for Transaction<'_> {}
impl DocTransactionExtension for TransactionMut<'_> {}
