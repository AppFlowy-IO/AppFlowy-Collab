use crate::error::DocumentError;
use collab::core::text_wrapper::TextRefWrapper;
use collab::preclude::*;
use nanoid::nanoid;
use std::ops::Deref;

const ROOT: &str = "document";
const ATTRIBUTES: &str = "attributes";
const META: &str = "meta";

pub struct Document {
    inner: Collab,
    root: MapRefWrapper,
    attributes: MapRefWrapper,
    meta: MapRefWrapper,
}

impl Document {
    pub fn create(collab: Collab) -> Self {
        let (root, attributes, meta) = collab.with_transact_mut(|txn| {
            let root = collab
                .get_map_with_txn(txn, vec![ROOT])
                .unwrap_or_else(|| collab.create_map_with_txn(txn, ROOT));
            let attributes = collab
                .get_map_with_txn(txn, vec![ROOT, ATTRIBUTES])
                .unwrap_or_else(|| root.create_map_with_txn(txn, ATTRIBUTES));
            let meta = collab
                .get_map_with_txn(txn, vec![ROOT, META])
                .unwrap_or_else(|| root.create_map_with_txn(txn, META));
            (root, attributes, meta)
        });

        Self {
            inner: collab,
            root,
            attributes,
            meta,
        }
    }

    pub fn attrs(&self) -> AttributeMap {
        AttributeMap(&self.attributes)
    }

    pub fn meta(&self) -> MetaMap {
        MetaMap(&self.meta)
    }

    pub fn to_json(&self) -> Result<String, DocumentError> {
        Ok(self.root.to_json())
    }
}

pub struct AttributeMap<'a>(&'a MapRefWrapper);
impl<'a> AttributeMap<'a> {
    pub fn insert_with_key<B>(&self, key: &str, f: B)
    where
        B: FnOnce(AttributeBuilder) -> MapRefWrapper,
    {
        self.0.with_transact_mut(|txn| {
            let builder = AttributeBuilder::new_with_txn(txn, key.to_string(), &self.0);
            let _ = f(builder);
        })
    }

    pub fn insert<B>(&self, f: B)
    where
        B: FnOnce(AttributeBuilder) -> MapRefWrapper,
    {
        self.0.with_transact_mut(|txn| {
            let builder = AttributeBuilder::new(txn, &self.0);
            let _ = f(builder);
        })
    }
}

impl<'a> Deref for AttributeMap<'a> {
    type Target = MapRefWrapper;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct MetaMap<'a>(&'a MapRefWrapper);
impl<'a> MetaMap<'a> {}

pub struct AttributeBuilder<'a, 'b> {
    key: String,
    map: MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> AttributeBuilder<'a, 'b> {
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

    pub fn with_data<V: Prelim>(self, data: V) -> Self {
        self.map.insert_with_txn(self.txn, "data", data);
        self
    }

    pub fn with_text_data<T: AsRef<str>>(self, data: T) -> Self {
        let text_ref = self.map.insert_text_with_txn(self.txn, "data");
        text_ref.insert(self.txn, 0, data.as_ref());
        self
    }

    pub fn with_next<T: AsRef<str>>(self, next: T) -> Self {
        self.map.insert_with_txn(self.txn, "next", next.as_ref());
        self
    }

    pub fn with_child<T: AsRef<str>>(self, child: T) -> Self {
        self.map.insert_with_txn(self.txn, "child", child.as_ref());
        self
    }

    pub fn build(self) -> MapRefWrapper {
        self.map
    }
}
