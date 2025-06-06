use std::collections::HashMap;

use collab_document::block_parser::parsers::heading::HeadingParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::{Value, json};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_heading_block(
  test: &mut BlockTestCore,
  level: u8,
  text: String,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  data.insert("level".to_string(), Value::String(level.to_string()));

  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::Heading.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_heading_parser_different_levels() {
  let mut test = BlockTestCore::new();
  let parser = HeadingParser;

  for level in 1..=6 {
    let block = create_heading_block(&mut test, level, format!("Level {} Heading", level), "");

    let document_data = test.get_document_data();
    let document_parser = DocumentParser::with_default_parsers();
    let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
    let result = parser.parse(&block, &context).unwrap();
    let expected = format!("{} Level {} Heading", "#".repeat(level as usize), level);
    assert_eq!(result.content, expected);
  }
}

#[test]
fn test_heading_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = HeadingParser;

  let block = create_heading_block(&mut test, 3, "Heading".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "Heading");
}

#[test]
fn test_heading_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = HeadingParser;

  let block = create_heading_block(&mut test, 2, "".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "## ");
}

#[test]
fn test_heading_parser_missing_level_defaults_to_1() {
  let mut test = BlockTestCore::new();
  let parser = HeadingParser;

  let data = HashMap::new();

  let delta = json!([{ "insert": "No Level" }]).to_string();
  let external_id = test.create_text(delta);

  let page_id = test.get_page().id;
  let block = Block {
    id: generate_id(),
    ty: BlockType::Heading.as_str().to_string(),
    parent: page_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block.clone(), None).unwrap();

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "# No Level");
}
