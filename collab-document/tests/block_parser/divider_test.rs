use std::collections::HashMap;

use collab_document::block_parser::parsers::divider::DividerParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_divider_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  let data = HashMap::new();

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::Divider.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_divider_parser_markdown_format() {
  let mut test = BlockTestCore::new();
  let parser = DividerParser;

  let block = create_divider_block(&mut test, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "---");
}

#[test]
fn test_divider_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = DividerParser;

  let block = create_divider_block(&mut test, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "---");
}

#[test]
fn test_divider_parser_with_indent() {
  let mut test = BlockTestCore::new();
  let parser = DividerParser;

  let block = create_divider_block(&mut test, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context =
    ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown).with_depth(2);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "    ---");
}
