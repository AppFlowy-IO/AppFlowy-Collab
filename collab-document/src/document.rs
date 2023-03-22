use crate::error::DocumentError;
use collab::preclude::*;
use nanoid::nanoid;

const ROOT: &str = "document";
const ATTRIBUTES: &str = "attributes";

pub struct Document {
    inner: Collab,
    container: MapRefWrapper,
}

impl Document {
    pub fn create(collab: Collab) -> Self {
        let container = collab.with_transact_mut(|txn| {
            // Create the document if it's not exist.
            match collab.get_map_with_txn(txn, vec![ROOT]) {
                None => build_document(&collab, txn),
                Some(container) => container,
            }
        });
        Self {
            inner: collab,
            container,
        }
    }

    pub fn insert<B>(&self, key: &str, f: B)
    where
        B: FnOnce(AttributeEntryBuilder) -> MapRefWrapper,
    {
        self.inner.with_transact_mut(|txn| {
            let builder =
                AttributeEntryBuilder::new_with_txn(txn, key.to_string(), &self.container);
            let _ = f(builder);
        })
    }

    pub fn to_json(&self) -> Result<String, DocumentError> {
        Ok(self.container.to_json())
    }
}

fn build_document(collab: &Collab, txn: &mut TransactionMut) -> MapRefWrapper {
    let document_map = collab.create_map_with_txn(txn, ROOT);
    let attributes_map = document_map.create_map_with_txn(txn, ATTRIBUTES);
    let _first_attribute = AttributeEntryBuilder::new(txn, &attributes_map)
        .with_type("text")
        .with_data("hello world")
        .build();
    document_map
}

pub struct AttributeEntryBuilder<'a, 'b> {
    key: String,
    map: MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> AttributeEntryBuilder<'a, 'b> {
    pub fn new(txn: &'a mut TransactionMut<'b>, container: &MapRefWrapper) -> Self {
        let key = nanoid!(4);
        Self::new_with_txn(txn, key, container)
    }

    pub fn new_with_txn(
        txn: &'a mut TransactionMut<'b>,
        key: String,
        container: &MapRefWrapper,
    ) -> Self {
        let map = match container.get_map_with_txn(txn, &key) {
            None => container.create_map_with_txn(txn, &key),
            Some(map) => map,
        };
        Self { key, map, txn }
    }

    pub fn with_type<T: AsRef<str>>(self, ty: T) -> Self {
        self.map.insert_with_txn(self.txn, "type", ty.as_ref());
        self
    }

    pub fn with_data<T: AsRef<str>>(self, data: T) -> Self {
        let text_ref = self.map.insert_text_with_txn(self.txn, "data");
        text_ref.insert(self.txn, 0, data.as_ref());
        self
    }

    // pub fn modify_data<>

    pub fn with_next<T: AsRef<str>>(self, next: T) -> Self {
        self.map.insert_with_txn(self.txn, "next", next.as_ref());
        self
    }

    pub fn with_child<T: AsRef<str>>(self, child: T) -> Self {
        self.map.insert_with_txn(self.txn, "child", child.as_ref());
        self
    }

    fn build(self) -> MapRefWrapper {
        self.map
    }
}
