use std::collections::HashMap;

use collab_document::block_parser::parsers::file_block::FileBlockParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::Value;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_file_block(
  test: &mut BlockTestCore,
  name: Option<String>,
  url: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(name) = name {
    data.insert("name".to_string(), Value::String(name));
  }
  if let Some(url) = url {
    data.insert("url".to_string(), Value::String(url));
  }

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::File.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_file_block_parser_with_url_markdown() {
  let mut test = BlockTestCore::new();
  let parser = FileBlockParser;

  let name = "document.pdf".to_string();
  let url = "https://appflowy.io/document.pdf".to_string();
  let block = create_file_block(&mut test, Some(name.clone()), Some(url.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("[{}]({})", name, url);
  assert_eq!(result.content, expected);
}

#[test]
fn test_file_block_parser_without_url() {
  let mut test = BlockTestCore::new();
  let parser = FileBlockParser;

  let name = "document.pdf".to_string();
  let block = create_file_block(&mut test, Some(name.clone()), None, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, name);
}

#[test]
fn test_file_block_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = FileBlockParser;

  let name = "document.pdf".to_string();
  let url = "https://appflowy.io/document.pdf".to_string();
  let block = create_file_block(&mut test, Some(name.clone()), Some(url.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("{}({})", name, url);
  assert_eq!(result.content, expected);
}
