use crate::document::BlocksMap;
use collab::preclude::{Attrs, Text, TextRefWrapper};

pub enum TextAction {
    Insert { index: u32, s: String },
    InsertWithAttribute { index: u32, s: String, attrs: Attrs },
    Remove { index: u32, len: u32 },
    Format { index: u32, len: u32, attrs: Attrs },
    Push { s: String },
}

pub fn get_text(blocks: BlocksMap, block_id: &str, text_id: &str) -> Option<TextRefWrapper> {
    let block = blocks.get_block(block_id)?;
    let txn = block.transact();
    let text = block.get_data(&txn);
    drop(txn);

    None
}

pub fn apply_text_actions(text_ref: TextRefWrapper, actions: Vec<TextAction>) {
    text_ref.with_transact_mut(|txn| {
        for action in actions {
            match action {
                TextAction::Insert { index, s } => {
                    text_ref.insert(txn, index, &s);
                }
                TextAction::InsertWithAttribute { index, s, attrs } => {
                    text_ref.insert_with_attributes(txn, index, &s, attrs)
                }
                TextAction::Remove { index, len } => {
                    text_ref.remove_range(txn, index, len);
                }
                TextAction::Format { index, len, attrs } => text_ref.format(txn, index, len, attrs),
                TextAction::Push { s } => {
                    text_ref.push(txn, &s);
                }
            }
        }
    });
}
