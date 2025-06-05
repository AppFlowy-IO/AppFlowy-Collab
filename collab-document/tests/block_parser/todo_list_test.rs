use std::collections::HashMap;

use collab_document::block_parser::parsers::todo_list::TodoListParser;
use collab_document::block_parser::{BlockParser, OutputFormat, ParseContext};
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
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Unchecked task");
}

#[test]
fn test_todo_list_parser_checked_markdown() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Completed task".to_string(), Some(true), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [x] Completed task");
}

#[test]
fn test_todo_list_parser_unchecked_plain_text() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Unchecked task".to_string(), Some(false), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "☐ Unchecked task");
}

#[test]
fn test_todo_list_parser_checked_plain_text() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Completed task".to_string(), Some(true), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "☑ Completed task");
}

#[test]
fn test_todo_list_parser_no_checked_data_defaults_unchecked() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Default task".to_string(), None, "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

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
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

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
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Boolean unchecked task");
}

#[test]
fn test_todo_list_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "".to_string(), Some(false), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] ");
}

#[test]
fn test_todo_list_parser_with_indentation() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  let block = create_todo_list_block(&mut test, "Indented task".to_string(), Some(true), "");
  let document_data = test.get_document_data();

  // Create a context with depth 2 for indentation
  let context = ParseContext::new(&document_data, OutputFormat::Markdown).with_depth(2);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "    - [x] Indented task");
}

#[test]
fn test_todo_list_parser_with_children() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  // Create parent todo item
  let parent_block = create_todo_list_block(&mut test, "Parent task".to_string(), Some(false), "");

  // Create child todo item
  let _child_block = create_todo_list_block(
    &mut test,
    "Child task".to_string(),
    Some(true),
    &parent_block.id,
  );

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&parent_block, &context).unwrap();

  // Should contain both parent and child content
  assert!(result.content.contains("- [ ] Parent task"));
  assert!(result.content.contains("- [x] Child task"));
}

#[test]
fn test_todo_list_parser_block_type() {
  let parser = TodoListParser;
  assert_eq!(parser.block_type(), "todo_list");
}

#[test]
fn test_todo_list_parser_invalid_checked_data_type() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  // Test with number data type (should default to unchecked)
  let mut data = HashMap::new();
  data.insert(
    "checked".to_string(),
    Value::Number(serde_json::Number::from(42)),
  );

  let delta = json!([{ "insert": "Number checked data" }]).to_string();
  let external_id = test.create_text(delta);
  let page_id = test.get_page().id;

  let block = Block {
    id: generate_id(),
    ty: BlockType::TodoList.as_str().to_string(),
    parent: page_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block.clone(), None).unwrap();

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] Number checked data");
}

#[test]
fn test_todo_list_parser_string_value_ignored() {
  let mut test = BlockTestCore::new();
  let parser = TodoListParser;

  // Test with string value (should be ignored and default to unchecked)
  let mut data = HashMap::new();
  data.insert(
    "checked".to_string(),
    Value::String("any_string".to_string()),
  );

  let delta = json!([{ "insert": "String value ignored" }]).to_string();
  let external_id = test.create_text(delta);
  let page_id = test.get_page().id;

  let block = Block {
    id: generate_id(),
    ty: BlockType::TodoList.as_str().to_string(),
    parent: page_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block.clone(), None).unwrap();

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "- [ ] String value ignored");
}
