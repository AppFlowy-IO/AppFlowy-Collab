use std::collections::HashMap;

use collab_document::block_parser::parsers::link_preview::LinkPreviewParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::Value;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_link_preview_block(
  test: &mut BlockTestCore,
  url: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
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
    ty: BlockType::LinkPreview.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_link_preview_parser_with_url_markdown() {
  let mut test = BlockTestCore::new();
  let parser = LinkPreviewParser;

  let url = "https://appflowy.io".to_string();
  let block = create_link_preview_block(&mut test, Some(url.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("[{}]({})", url, url);
  assert_eq!(result.content, expected);
}

#[test]
fn test_link_preview_parser_empty_url() {
  let mut test = BlockTestCore::new();
  let parser = LinkPreviewParser;

  let block = create_link_preview_block(&mut test, None, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert!(result.content.is_empty());
}

#[test]
fn test_link_preview_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = LinkPreviewParser;

  let url = "https://appflowy.io".to_string();
  let block = create_link_preview_block(&mut test, Some(url.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, url);
}
