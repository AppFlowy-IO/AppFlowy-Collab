use crate::preclude::CollabContext;
use std::ops::Deref;
use yrs::types::Attrs;
use yrs::TextRef;

pub struct TextRefWrapper {
    text_ref: TextRef,
    collab_ctx: CollabContext,
}

impl TextRefWrapper {
    pub fn new(text_ref: TextRef, collab_ctx: CollabContext) -> Self {
        Self {
            text_ref,
            collab_ctx,
        }
    }

    pub fn apply_action(&self, action: TextAction) {
        match action {
            TextAction::Insert { .. } => {}
            TextAction::InsertWithAttribute { .. } => {}
            TextAction::Remove { .. } => {}
            TextAction::Format { .. } => {}
        }
    }
}

impl Deref for TextRefWrapper {
    type Target = TextRef;

    fn deref(&self) -> &Self::Target {
        &self.text_ref
    }
}

pub enum TextAction {
    Insert { index: u32, s: String },
    InsertWithAttribute { index: u32, s: String, attrs: Attrs },
    Remove { index: u32, len: u32 },
    Format { index: u32, len: u32, attrs: Attrs },
}
