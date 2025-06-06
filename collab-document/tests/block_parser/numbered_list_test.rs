use std::collections::HashMap;

use collab_document::block_parser::parsers::numbered_list::NumberedListParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::{Value, json};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_numbered_list_block(
  test: &mut BlockTestCore,
  number: Option<usize>,
  text: String,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(num) = number {
    data.insert(
      "number".to_string(),
      Value::Number(serde_json::Number::from(num)),
    );
  }

  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::NumberedList.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_numbered_list_block_with_string_number(
  test: &mut BlockTestCore,
  number: Option<&str>,
  text: String,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(num_str) = number {
    data.insert("number".to_string(), Value::String(num_str.to_string()));
  }

  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::NumberedList.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_numbered_list_parser_with_specific_number() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, Some(5), "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "5. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_without_number_defaults_to_1() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, None, "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "1. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, Some(3), "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "3. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_with_string_number() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block_with_string_number(
    &mut test,
    Some("7"),
    "Hello AppFlowy".to_string(),
    "",
  );
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "7. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_with_invalid_string_number() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block_with_string_number(
    &mut test,
    Some("invalid"),
    "Hello AppFlowy".to_string(),
    "",
  );
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "1. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_with_context_list_number() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, None, "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();

  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let context_with_list_number = context.with_list_context(Some(4));

  let result = parser.parse(&block, &context_with_list_number).unwrap();
  assert_eq!(result.content, "4. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, Some(2), "".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "2. ");
}

#[test]
fn test_numbered_list_parser_with_indentation() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, Some(1), "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();

  let document_parser = DocumentParser::with_default_parsers();
  let context =
    ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown).with_depth(2);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "    1. Hello AppFlowy");
}

#[test]
fn test_numbered_list_parser_with_children() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let parent_block =
    create_numbered_list_block(&mut test, Some(1), "Hello AppFlowy".to_string(), "");

  let _ = create_numbered_list_block(
    &mut test,
    Some(1),
    "Hello AppFlowy".to_string(),
    &parent_block.id,
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&parent_block, &context).unwrap();

  assert!(result.content.contains("1. Hello AppFlowy"));
  assert!(result.content.contains("1. Hello AppFlowy"));
}

#[test]
fn test_numbered_list_parser_increments_context_number() {
  let mut test = BlockTestCore::new();
  let parser = NumberedListParser;

  let block = create_numbered_list_block(&mut test, Some(5), "Hello AppFlowy".to_string(), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "5. Hello AppFlowy");
}
