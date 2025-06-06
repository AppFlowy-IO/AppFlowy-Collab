use std::collections::HashMap;

use collab_document::block_parser::parsers::todo_list::TodoListParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::{Value, json};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_todo_list_block(
  test: &mut BlockTestCore,
  text: String,
  checked: Option<bool>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(is_checked) = checked {
    data.insert("checked".to_string(), Value::Bool(is_checked));
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
    ty: BlockType::TodoList.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_todo_list_parser_unchecked_markdown() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Unchecked task".to_string(), Some(false), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Unchecked task");
}

#[test]
fn test_todo_list_parser_checked_markdown() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Completed task".to_string(), Some(true), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [x] Completed task");
}

#[test]
fn test_todo_list_parser_checked_plain_text() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Completed task".to_string(), Some(true), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "Completed task");
}

#[test]
fn test_todo_list_parser_no_checked_data_defaults_unchecked() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Default task".to_string(), None, "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Default task");
}

#[test]
fn test_todo_list_parser_with_bool_checked_true() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(
    &mut test,
    "Boolean checked task".to_string(),
    Some(true),
    "",
  );
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [x] Boolean checked task");
}

#[test]
fn test_todo_list_parser_with_bool_checked_false() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(
    &mut test,
    "Boolean unchecked task".to_string(),
    Some(false),
    "",
  );
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Boolean unchecked task");
}

#[test]
fn test_todo_list_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "".to_string(), Some(false), "");
  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] ");
}

#[test]
fn test_todo_list_parser_with_indentation() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Indented task".to_string(), Some(true), "");
  let document_data = test.get_document_data();

  let document_parser = DocumentParser::with_default_parsers();
  let context =
    ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown).with_depth(2);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "    - [x] Indented task");
}
