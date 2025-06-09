use collab_document::block_parser::{
  DocumentParser, DocumentParserDelegate, OutputFormat, ParseContext,
};
use collab_document::blocks::{Block, BlockType, mention_block_delta};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use yrs::{Any, types::Attrs};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

#[derive(Debug)]
struct MentionDelegate;

impl DocumentParserDelegate for MentionDelegate {
  fn handle_text_delta(
    &self,
    text: &str,
    attributes: Option<&Attrs>,
    _context: &ParseContext,
  ) -> Option<String> {
    if text != "$" {
      return None;
    }

    if let Some(attrs) = attributes {
      if let Some(Any::Map(values)) = attrs.get("mention") {
        if let Some(Any::String(page_id)) = values.get("page_id") {
          return Some(format!("[[{}]]", page_id));
        }
      }
    }

    None
  }
}

#[test]
fn test_document_parser_with_mention_delegate() {
  let delegate = Arc::new(MentionDelegate);
  let parser = DocumentParser::with_default_parsers().with_delegate(delegate);

  let mut test = BlockTestCore::new();
  let page = test.get_page();
  let page_id = page.id.as_str();

  let view_id = "test_page_id";
  let mention_delta = mention_block_delta(view_id);

  let delta_json = json!([
    {"insert": "Mention a page: "},
    mention_delta
  ])
  .to_string();

  let external_id = test.create_text(delta_json);

  let data = HashMap::new();
  let block = Block {
    id: generate_id(),
    ty: BlockType::Paragraph.as_str().to_string(),
    parent: page_id.to_string(),
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap();

  let document_data = test.get_document_data();
  let result = parser
    .parse_document(&document_data, OutputFormat::PlainText)
    .unwrap();

  let expected = format!("Mention a page: [[{}]]", view_id);
  assert_eq!(result.trim(), expected);

  let result_md = parser
    .parse_document(&document_data, OutputFormat::Markdown)
    .unwrap();
  assert_eq!(result_md.trim(), expected);
}
