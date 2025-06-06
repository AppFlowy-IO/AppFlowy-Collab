use std::collections::HashMap;

use collab_document::block_parser::document_parser::DocumentParser;
use collab_document::block_parser::{OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::json;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_simple_table_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  let data = HashMap::new();

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::SimpleTable.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_simple_table_row_block(
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
    ty: BlockType::SimpleTableRow.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, prev_id).unwrap()
}

fn create_simple_table_cell_block(
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
    ty: BlockType::SimpleTableCell.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, prev_id).unwrap()
}

fn create_paragraph_block_in_cell(
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
fn test_simple_table_parser_with_single_row_and_multiple_cells() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let table_block = create_simple_table_block(&mut test, "");

  let row_block = create_simple_table_row_block(&mut test, &table_block.id, None);

  let cell1_block = create_simple_table_cell_block(&mut test, &row_block.id, None);
  let _paragraph1 =
    create_paragraph_block_in_cell(&mut test, "Cell 1".to_string(), &cell1_block.id, None);

  let cell2_block =
    create_simple_table_cell_block(&mut test, &row_block.id, Some(cell1_block.id.clone()));
  let _paragraph2 =
    create_paragraph_block_in_cell(&mut test, "Cell 2".to_string(), &cell2_block.id, None);

  let cell3_block =
    create_simple_table_cell_block(&mut test, &row_block.id, Some(cell2_block.id.clone()));
  let _paragraph3 =
    create_paragraph_block_in_cell(&mut test, "Cell 3".to_string(), &cell3_block.id, None);

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let table_result = parser.parse_block(&table_block, &context).unwrap();
  let expected = "Cell 1\tCell 2\tCell 3";
  assert_eq!(table_result, expected);
}

#[test]
fn test_simple_table_parser_with_multiple_rows_and_cells() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let table_block = create_simple_table_block(&mut test, "");

  let row1_block = create_simple_table_row_block(&mut test, &table_block.id, None);

  let cell1_1_block = create_simple_table_cell_block(&mut test, &row1_block.id, None);
  let _paragraph1_1 = create_paragraph_block_in_cell(
    &mut test,
    "Row 1, Cell 1".to_string(),
    &cell1_1_block.id,
    None,
  );

  let cell1_2_block =
    create_simple_table_cell_block(&mut test, &row1_block.id, Some(cell1_1_block.id.clone()));
  let _paragraph1_2 = create_paragraph_block_in_cell(
    &mut test,
    "Row 1, Cell 2".to_string(),
    &cell1_2_block.id,
    None,
  );

  let row2_block =
    create_simple_table_row_block(&mut test, &table_block.id, Some(row1_block.id.clone()));

  let cell2_1_block = create_simple_table_cell_block(&mut test, &row2_block.id, None);
  let _paragraph2_1 = create_paragraph_block_in_cell(
    &mut test,
    "Row 2, Cell 1".to_string(),
    &cell2_1_block.id,
    None,
  );

  let cell2_2_block =
    create_simple_table_cell_block(&mut test, &row2_block.id, Some(cell2_1_block.id.clone()));
  let _paragraph2_2 = create_paragraph_block_in_cell(
    &mut test,
    "Row 2, Cell 2".to_string(),
    &cell2_2_block.id,
    None,
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let table_result = parser.parse_block(&table_block, &context).unwrap();
  let expected = "Row 1, Cell 1\tRow 1, Cell 2\nRow 2, Cell 1\tRow 2, Cell 2";
  assert_eq!(table_result, expected);
}

#[test]
fn test_simple_table_parser_with_complex_cell_content() {
  let mut test = BlockTestCore::new();
  let parser = DocumentParser::with_default_parsers();

  let table_block = create_simple_table_block(&mut test, "");

  let row_block = create_simple_table_row_block(&mut test, &table_block.id, None);

  let cell_block = create_simple_table_cell_block(&mut test, &row_block.id, None);
  let paragraph1 = create_paragraph_block_in_cell(
    &mut test,
    "First paragraph in cell".to_string(),
    &cell_block.id,
    None,
  );
  let _paragraph2 = create_paragraph_block_in_cell(
    &mut test,
    "Second paragraph in cell".to_string(),
    &cell_block.id,
    Some(paragraph1.id),
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let table_result = parser.parse_block(&table_block, &context).unwrap();
  let expected = "First paragraph in cell\nSecond paragraph in cell";
  assert_eq!(table_result, expected);
}
