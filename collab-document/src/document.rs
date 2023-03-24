use crate::blocks::{Block, BlockBuilder, BlockMap, BlockMapRef, TextMap};
use crate::error::DocumentError;
use collab::preclude::*;

const ROOT: &str = "document";
const BLOCKS: &str = "blocks";
const TESTS: &str = "texts";

pub struct Document {
    inner: Collab,
    root: MapRefWrapper,
    pub blocks: BlockMap,
    pub texts: TextMap,
}

impl Document {
    pub fn create(collab: Collab) -> Self {
        let (root, blocks, texts) = collab.with_transact_mut(|txn| {
            let root = collab
                .get_map_with_txn(txn, vec![ROOT])
                .unwrap_or_else(|| collab.create_map_with_txn(txn, ROOT));
            let blocks = collab
                .get_map_with_txn(txn, vec![ROOT, BLOCKS])
                .unwrap_or_else(|| root.insert_map_with_txn(txn, BLOCKS));
            let texts = collab
                .get_map_with_txn(txn, vec![ROOT, TESTS])
                .unwrap_or_else(|| root.insert_map_with_txn(txn, TESTS));
            (root, blocks, texts)
        });
        let blocks = BlockMap::new(blocks);
        let texts = TextMap::new(texts);
        Self {
            inner: collab,
            root,
            blocks,
            texts,
        }
    }

    pub fn to_json(&self) -> Result<String, DocumentError> {
        Ok(self.root.to_json())
    }
}
