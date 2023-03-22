use collab::preclude::{Collab, CollabBuilder, TransactionMut};
use collab_derive::Collab;
use std::collections::HashMap;

pub struct Document {
    inner: Collab,
}

impl Document {
    pub fn create(collab: Collab) -> Self {
        collab.with_transact_mut(|txn| {
            build_document_struct(&collab, txn);
        });
        Self { inner: collab }
    }
}

fn build_document_struct(collab: &Collab, txn: &mut TransactionMut) {}

#[derive(Collab, Serialize, Deserialize, Clone)]
struct DocumentLayout {
    id: String,
    name: String,
    created_at: i64,
    attributes: HashMap<String, String>,
}
