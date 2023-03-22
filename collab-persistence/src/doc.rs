use crate::keys::{
    doc_name_from_key, make_doc_id, make_doc_state_key, make_state_vector_key, make_update_key,
    Key, DID, DID_SPACE, DOC_SPACE, SPACE,
};
use crate::{CLError, CollabKV};
use sled::{IVec, Iter};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ReadTxn, StateVector, TransactionMut, Update};

pub struct YrsDoc<'a> {
    pub(crate) db: &'a CollabKV,
}

impl<'a> YrsDoc<'a> {
    pub fn create_new_doc<K: AsRef<[u8]>, T: ReadTxn>(
        &self,
        name: &K,
        txn: &T,
    ) -> Result<(), CLError> {
        let doc_state = txn.encode_diff_v1(&StateVector::default());
        let sv = txn.state_vector().encode_v1();
        let did = self.get_or_create_did(name.as_ref())?;
        let doc_state_key = make_doc_state_key(did);
        let sv_key = make_state_vector_key(did);
        self.db.insert(&doc_state_key, &doc_state)?;
        self.db.insert(&sv_key, &sv)?;
        Ok(())
    }

    pub fn load_doc<K: AsRef<[u8]>>(
        &self,
        name: &K,
        txn: &mut TransactionMut,
    ) -> Result<(), CLError> {
        if let Some(did) = self.get_did(name) {
            let doc_state_key = make_doc_state_key(did);
            if let Some(doc_state) = self.db.get(doc_state_key)? {
                let update = Update::decode_v1(doc_state.as_ref())?;
                txn.apply_update(update);
            }

            let update_start = make_update_key(did, 0);
            let update_end = make_update_key(did, u32::MAX);
            let mut encoded_updates = self.db.batch_get(&update_start, &update_end)?;
            for encoded_update in encoded_updates {
                let update = Update::decode_v1(encoded_update.as_ref())?;
                txn.apply_update(update);
            }
            Ok(())
        } else {
            Err(CLError::DocumentNotExist)
        }
    }

    pub fn push_update<K: AsRef<[u8]> + ?Sized>(
        &self,
        name: &K,
        update: &[u8],
    ) -> Result<(), CLError> {
        let did = self.get_or_create_did(name.as_ref())?;
        let last_clock = {
            let end = make_update_key(did, u32::MAX);
            if let Some((k, _v)) = self.peek_back(&end) {
                let last_key = k.as_ref();
                let len = last_key.len();
                let last_clock = &last_key[(len - 5)..(len - 1)]; // update key scheme: 01{name:n}1{clock:4}0
                u32::from_be_bytes(last_clock.try_into().unwrap())
            } else {
                0
            }
        };
        let clock = last_clock + 1;
        let update_key = make_update_key(did, clock);
        self.db.insert(&update_key, &update)?;
        Ok(())
    }

    pub fn delete_doc<K: AsRef<[u8]>>(&self, name: &K) -> Result<(), CLError> {
        todo!()
    }

    pub fn get_all_docs(&self) -> Result<DocsNameIter, CLError> {
        let from = Key::from_const([SPACE, DID_SPACE]);
        let to = Key::from_const([SPACE, DOC_SPACE]);
        let iter = self.db.range(from..=to);

        Ok(DocsNameIter { iter })
    }

    fn get_or_create_did<K: AsRef<[u8]> + ?Sized>(&self, name: &K) -> Result<DID, CLError> {
        if let Some(did) = self.get_did(name.as_ref()) {
            Ok(did)
        } else {
            let last_did = self.peek_back_did([SPACE, DOC_SPACE].as_ref()).unwrap_or(0);
            let new_did = last_did + 1;
            let key = make_doc_id(name.as_ref());
            let _ = self.db.insert(key, &new_did.to_be_bytes());
            Ok(new_did)
        }
    }

    fn get_did<K: AsRef<[u8]> + ?Sized>(&self, name: &K) -> Option<DID> {
        let key = make_doc_id(name.as_ref());
        let value = self.db.get(key).ok()??;
        Some(DID::from_be_bytes(value.as_ref().try_into().unwrap()))
    }

    /// Looks into the last entry value prior to a given key.
    fn peek_back(&self, key: &[u8]) -> Option<(IVec, IVec)> {
        let (k, v) = self.db.get_gt(key).ok()??;
        Some((k, v))
    }

    fn peek_back_did(&self, key: &[u8]) -> Option<DID> {
        let (_, v) = self.peek_back(key)?;
        Some(DID::from_be_bytes(v.as_ref().try_into().ok()?))
    }
}

pub struct DocsNameIter {
    iter: Iter,
}

impl Iterator for DocsNameIter {
    type Item = Box<[u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        let (k, _) = self.iter.next()?.ok()?;
        Some(doc_name_from_key(k.as_ref()).into())
    }
}
