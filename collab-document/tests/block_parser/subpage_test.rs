use std::collections::HashMap;

use collab_document::block_parser::parsers::subpage::SubpageParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::Value;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_subpage_block(
  test: &mut BlockTestCore,
  view_id: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(view_id) = view_id {
    data.insert("viewId".to_string(), Value::String(view_id));
  }

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::SubPage.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_subpage_parser_with_view_id_markdown() {
  let mut test = BlockTestCore::new();
  let parser = SubpageParser;

  let view_id = "page_id".to_string();
  let block = create_subpage_block(&mut test, Some(view_id.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("[Subpage]({})", view_id);
  assert_eq!(result.content, expected);
}

#[test]
fn test_subpage_parser_without_view_id() {
  let mut test = BlockTestCore::new();
  let parser = SubpageParser;

  let block = create_subpage_block(&mut test, None, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "[Subpage]");
}

#[test]
fn test_subpage_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = SubpageParser;

  let view_id = "page_id".to_string();
  let block = create_subpage_block(&mut test, Some(view_id.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, view_id);
}
