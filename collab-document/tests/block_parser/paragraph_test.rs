use std::collections::HashMap;

use collab_document::block_parser::parsers::paragraph::ParagraphParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_paragraph_block(test: &mut BlockTestCore, text: String, parent_id: &str) -> Block {
  let data = HashMap::new();

  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::Paragraph.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_paragraph_parser_basic_text() {
  let mut test = BlockTestCore::new();
  let parser = ParagraphParser;

  let block = create_paragraph_block(&mut test, "Hello AppFlowy".to_string(), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "Hello AppFlowy");
}

#[test]
fn test_paragraph_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = ParagraphParser;

  let block = create_paragraph_block(&mut test, "".to_string(), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert!(result.content.is_empty());
}
