use std::collections::HashMap;

use collab_document::block_parser::parsers::quote_list::QuoteListParser;
use collab_document::block_parser::{BlockParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_quote_list_block(test: &mut BlockTestCore, text: String, parent_id: &str) -> Block {
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
    ty: BlockType::Quote.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_quote_list_parser_markdown_format() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(&mut test, "This is a quote".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "> This is a quote");
}

#[test]
fn test_quote_list_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(&mut test, "This is a quote".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::PlainText);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "\" This is a quote\"");
}

#[test]
fn test_quote_list_parser_empty_content() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(&mut test, "".to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "> ");
}

#[test]
fn test_quote_list_parser_with_indentation() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(&mut test, "Indented quote".to_string(), "");
  let document_data = test.get_document_data();

  // Create a context with depth 2 for indentation
  let context = ParseContext::new(&document_data, OutputFormat::Markdown).with_depth(2);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "    > Indented quote");
}

#[test]
fn test_quote_list_parser_with_children() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  // Create parent quote
  let parent_block = create_quote_list_block(&mut test, "Parent quote".to_string(), "");

  // Create child quote
  let _child_block =
    create_quote_list_block(&mut test, "Child quote".to_string(), &parent_block.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&parent_block, &context).unwrap();

  // Should contain both parent and child content
  assert!(result.content.contains("> Parent quote"));
  assert!(result.content.contains("> Child quote"));
}

#[test]
fn test_quote_list_parser_block_type() {
  let parser = QuoteListParser;
  assert_eq!(parser.block_type(), "quote");
}

#[test]
fn test_quote_list_parser_multiple_quotes() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block1 = create_quote_list_block(&mut test, "First quote".to_string(), "");
  let block2 = create_quote_list_block(&mut test, "Second quote".to_string(), "");

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result1 = parser.parse(&block1, &context).unwrap();
  let result2 = parser.parse(&block2, &context).unwrap();

  assert_eq!(result1.content, "> First quote");
  assert_eq!(result2.content, "> Second quote");
}

#[test]
fn test_quote_list_parser_special_characters() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(
    &mut test,
    "Quote with *special* characters & symbols".to_string(),
    "",
  );
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(
    result.content,
    "> Quote with *special* characters & symbols"
  );
}

#[test]
fn test_quote_list_parser_nested_indentation() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let block = create_quote_list_block(&mut test, "Deeply nested quote".to_string(), "");
  let document_data = test.get_document_data();

  // Create a context with depth 3 for deeper indentation
  let context = ParseContext::new(&document_data, OutputFormat::PlainText).with_depth(3);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, "      \" Deeply nested quote\"");
}

#[test]
fn test_quote_list_parser_long_quote() {
  let mut test = BlockTestCore::new();
  let parser = QuoteListParser;

  let long_text = "This is a very long quote that spans multiple words and contains various punctuation marks, numbers like 123, and other content to test how the parser handles longer text content.";
  let block = create_quote_list_block(&mut test, long_text.to_string(), "");
  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, OutputFormat::Markdown);

  let result = parser.parse(&block, &context).unwrap();
  assert_eq!(result.content, format!("> {}", long_text));
}
