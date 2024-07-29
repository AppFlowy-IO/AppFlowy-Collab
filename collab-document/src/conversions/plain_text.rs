use crate::{document::Document, error::DocumentError};

pub fn convert_document_to_plain_text(document: Document) -> Result<String, DocumentError> {
  let page_id = document.get_page_id();
  match page_id {
    Some(page_id) => {
      let plain_text = get_plain_text_from_block(&document, &page_id);
      Ok(plain_text)
    },
    None => Err(DocumentError::ParseDocumentError),
  }
}

fn get_plain_text_from_block(document: &Document, block_id: &str) -> String {
  let mut text = document
    .get_plain_text_from_block(block_id)
    .unwrap_or_default();
  let children = document.get_block_children(block_id);

  for child_id in children {
    text.push('\n');
    text.push_str(&get_plain_text_from_block(document, &child_id));
  }

  text
}
