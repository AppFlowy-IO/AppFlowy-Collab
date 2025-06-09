use std::collections::HashMap;

use collab_document::block_parser::parsers::image::ImageParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::Value;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_image_block(test: &mut BlockTestCore, url: &str, parent_id: &str) -> Block {
  let mut data = HashMap::new();
  data.insert("url".to_string(), Value::String(url.to_string()));

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::Image.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_image_parser_markdown_format() {
  let mut test = BlockTestCore::new();
  let parser = ImageParser;

  let block = create_image_block(&mut test, "https://appflowy.io/image.png", "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "![Image](https://appflowy.io/image.png)");
}

#[test]
fn test_image_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = ImageParser;

  let block = create_image_block(&mut test, "https://appflowy.io/image.png", "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "https://appflowy.io/image.png");
}

#[test]
fn test_image_parser_empty_url_markdown() {
  let mut test = BlockTestCore::new();
  let parser = ImageParser;

  let block = create_image_block(&mut test, "", "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "![Image]()");
}

#[test]
fn test_image_parser_empty_url_plain_text() {
  let mut test = BlockTestCore::new();
  let parser = ImageParser;

  let block = create_image_block(&mut test, "", "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "");
}

#[test]
fn test_image_parser_missing_url_data() {
  let mut test = BlockTestCore::new();
  let parser = ImageParser;

  let data = HashMap::new();

  let page_id = test.get_page().id;
  let block = Block {
    id: generate_id(),
    ty: BlockType::Image.as_str().to_string(),
    parent: page_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block.clone(), None).unwrap();

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "![Image]()");
}
