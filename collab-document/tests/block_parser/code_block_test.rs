use std::collections::HashMap;

use collab_document::block_parser::parsers::code_block::CodeBlockParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::{Value, json};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_code_block(
  test: &mut BlockTestCore,
  code: String,
  language: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(lang) = language {
    data.insert("language".to_string(), Value::String(lang));
  }

  let delta = json!([{ "insert": code }]).to_string();
  let external_id = test.create_text(delta);

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::Code.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_code_block_parser_with_language() {
  let mut test = BlockTestCore::new();
  let parser = CodeBlockParser;

  let code = "print('Hello AppFlowy');".to_string();
  let block = create_code_block(&mut test, code.clone(), Some("dart".to_string()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("```dart\n{}\n```", code);
  assert_eq!(result.content, expected);
}

#[test]
fn test_code_block_parser_without_language() {
  let mut test = BlockTestCore::new();
  let parser = CodeBlockParser;

  let code = "print('Hello AppFlowy');".to_string();
  let block = create_code_block(&mut test, code.clone(), None, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("```\n{}\n```", code);
  assert_eq!(result.content, expected);
}

#[test]
fn test_code_block_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = CodeBlockParser;

  let code = "print('Hello AppFlowy')".to_string();
  let block = create_code_block(&mut test, code.clone(), Some("dart".to_string()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, code);
}
