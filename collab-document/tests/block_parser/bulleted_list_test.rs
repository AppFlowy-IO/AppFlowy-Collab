use std::collections::HashMap;

use collab_document::block_parser::parsers::bulleted_list::BulletedListParser;
use collab_document::block_parser::{BlockParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_bulleted_list_block(test: &mut BlockTestCore, text: String, parent_id: &str) -> Block {
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
    ty: BlockType::BulletedList.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_bulleted_list_parser_markdown_format() {
  let mut test = BlockTestCore::new();
  let parser = BulletedListParser;

  let block = create_bulleted_list_block(&mut test, "First item".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "* First item");
}

#[test]
fn test_bulleted_list_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = BulletedListParser;

  let block = create_bulleted_list_block(&mut test, "First item".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "• First item");
}

#[test]
fn test_bulleted_list_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = BulletedListParser;

  let block = create_bulleted_list_block(&mut test, "".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "* ");
}

#[test]
fn test_bulleted_list_parser_with_indentation() {
  let mut test = BlockTestCore::new();
  let parser = BulletedListParser;

  let block = create_bulleted_list_block(&mut test, "Indented item".to_string(), "");
  let document_data = test.get_document_data();

  // Create a context with depth 2 for indentation
  let context = ParseContext::new(&document_data, OutputFormat::Markdown).with_depth(2);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "    * Indented item");
}

#[test]
fn test_bulleted_list_parser_nested_indentation() {
  let mut test = BlockTestCore::new();
  let parser = BulletedListParser;

  let block = create_bulleted_list_block(&mut test, "Deeply nested item".to_string(), "");
  let document_data = test.get_document_data();

  // Create a context with depth 3 for deeper indentation
  let context = ParseContext::new(&document_data, OutputFormat::PlainText).with_depth(3);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "      • Deeply nested item");
}
