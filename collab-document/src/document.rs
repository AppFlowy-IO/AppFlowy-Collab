use crate::block::{Block, BlockBuilder, BlockMapRef};
use crate::error::DocumentError;
use collab::preclude::*;
use std::ops::Deref;

const ROOT: &str = "document";
const BLOCKS: &str = "blocks";
const META: &str = "meta";

pub struct Document {
    inner: Collab,
    root: MapRefWrapper,
    blocks: MapRefWrapper,
    meta: MapRefWrapper,
}

impl Document {
    pub fn create(collab: Collab) -> Self {
        let (root, blocks, meta) = collab.with_transact_mut(|txn| {
            let root = collab
                .get_map_with_txn(txn, vec![ROOT])
                .unwrap_or_else(|| collab.create_map_with_txn(txn, ROOT));
            let blocks = collab
                .get_map_with_txn(txn, vec![ROOT, BLOCKS])
                .unwrap_or_else(|| root.create_map_with_txn(txn, BLOCKS));
            let meta = collab
                .get_map_with_txn(txn, vec![ROOT, META])
                .unwrap_or_else(|| root.create_map_with_txn(txn, META));
            (root, blocks, meta)
        });

        Self {
            inner: collab,
            root,
            blocks,
            meta,
        }
    }

    pub fn blocks(&self) -> BlocksMap {
        BlocksMap(&self.blocks)
    }

    pub fn meta(&self) -> MetaMap {
        MetaMap(&self.meta)
    }

    pub fn to_json(&self) -> Result<String, DocumentError> {
        Ok(self.root.to_json())
    }
}

pub struct MetaMap<'a>(&'a MapRefWrapper);
impl<'a> MetaMap<'a> {}

pub struct BlocksMap<'a>(&'a MapRefWrapper);
impl<'a> BlocksMap<'a> {
    pub fn get_block(&self, block_id: &str) -> Option<BlockMapRef> {
        let txn = self.0.transact();
        let map_ref = self.0.get_map_with_txn(&txn, block_id)?;
        let block_map = BlockMapRef::from_map_ref(map_ref);
        drop(txn);
        Some(block_map)
    }

    pub fn create_block<B>(&self, block_id: &str, f: B)
    where
        B: FnOnce(BlockBuilder) -> BlockMapRef,
    {
        self.0.with_transact_mut(|txn| {
            let builder = BlockBuilder::new_with_txn(txn, block_id.to_string(), &self.0);
            let _ = f(builder);
        })
    }
}
impl<'a> Deref for BlocksMap<'a> {
    type Target = MapRefWrapper;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
