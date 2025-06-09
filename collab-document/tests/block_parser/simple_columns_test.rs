use std::collections::HashMap;

use collab_document::block_parser::document_parser::DocumentParser;
use collab_document::block_parser::{OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_simple_columns_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  let data = HashMap::new();

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::SimpleColumns.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_simple_column_block(
  test: &mut BlockTestCore,
  parent_id: &str,
  prev_id: Option<String>,
) -> Block {
  let data = HashMap::new();

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::SimpleColumn.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, prev_id).unwrap()
}

fn create_paragraph_block_in_column(
  test: &mut BlockTestCore,
  text: String,
  parent_id: &str,
  prev_id: Option<String>,
) -> Block {
  let data = HashMap::new();

  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);

  let block = Block {
    id: generate_id(),
    ty: BlockType::Paragraph.as_str().to_string(),
    parent: parent_id.to_string(),
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, prev_id).unwrap()
}

#[test]
fn test_simple_columns_parser_with_multiple_columns() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let columns_block = create_simple_columns_block(&mut test, "");

  let column1_block = create_simple_column_block(&mut test, &columns_block.id, None);
  let _paragraph1 = create_paragraph_block_in_column(
    &mut test,
    "1. Hello AppFlowy".to_string(),
    &column1_block.id,
    None,
  );

  let column2_block =
    create_simple_column_block(&mut test, &columns_block.id, Some(column1_block.id.clone()));
  let _paragraph2 = create_paragraph_block_in_column(
    &mut test,
    "2. Hello AppFlowy".to_string(),
    &column2_block.id,
    None,
  );

  let column3_block =
    create_simple_column_block(&mut test, &columns_block.id, Some(column2_block.id.clone()));
  let _paragraph3 = create_paragraph_block_in_column(
    &mut test,
    "3. Hello AppFlowy".to_string(),
    &column3_block.id,
    None,
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let columns_result = parser.parse_block(&columns_block, &context).unwrap();
  let expected = "1. Hello AppFlowy\n2. Hello AppFlowy\n3. Hello AppFlowy";
  assert_eq!(columns_result, expected);
}

#[test]
fn test_simple_columns_parser_with_multiple_paragraphs_per_column() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let columns_block = create_simple_columns_block(&mut test, "");

  let column1_block = create_simple_column_block(&mut test, &columns_block.id, None);
  let paragraph1a = create_paragraph_block_in_column(
    &mut test,
    "1. Hello AppFlowy - Line 1".to_string(),
    &column1_block.id,
    None,
  );
  let _paragraph1b = create_paragraph_block_in_column(
    &mut test,
    "1. Hello AppFlowy - Line 2".to_string(),
    &column1_block.id,
    Some(paragraph1a.id),
  );

  let column2_block =
    create_simple_column_block(&mut test, &columns_block.id, Some(column1_block.id.clone()));
  let paragraph2a = create_paragraph_block_in_column(
    &mut test,
    "2. Hello AppFlowy - Line 1".to_string(),
    &column2_block.id,
    None,
  );
  let _paragraph2b = create_paragraph_block_in_column(
    &mut test,
    "2. Hello AppFlowy - Line 2".to_string(),
    &column2_block.id,
    Some(paragraph2a.id),
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let columns_result = parser.parse_block(&columns_block, &context).unwrap();
  let expected = "1. Hello AppFlowy - Line 1\n1. Hello AppFlowy - Line 2\n2. Hello AppFlowy - Line 1\n2. Hello AppFlowy - Line 2";
  assert_eq!(columns_result, expected);
}

#[test]
fn test_simple_columns_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let columns_block = create_simple_columns_block(&mut test, "");

  let column1_block = create_simple_column_block(&mut test, &columns_block.id, None);
  let _paragraph1 = create_paragraph_block_in_column(
    &mut test,
    "1. Hello AppFlowy".to_string(),
    &column1_block.id,
    None,
  );

  let column2_block =
    create_simple_column_block(&mut test, &columns_block.id, Some(column1_block.id.clone()));
  let _paragraph2 = create_paragraph_block_in_column(
    &mut test,
    "2. Hello AppFlowy".to_string(),
    &column2_block.id,
    None,
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let columns_result = parser.parse_block(&columns_block, &context).unwrap();
  let expected = "1. Hello AppFlowy\n2. Hello AppFlowy";
  assert_eq!(columns_result, expected);
}
