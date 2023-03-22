use yrs::TransactionMut;

pub trait CollabPlugin: Send + Sync + 'static {
    fn did_init(&self, _cid: &str, _txn: &mut TransactionMut) {}
    fn did_receive_update(&self, cid: &str, txn: &TransactionMut, update: &[u8]);
}
