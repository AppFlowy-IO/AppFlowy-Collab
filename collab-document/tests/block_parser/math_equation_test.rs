use std::collections::HashMap;

use collab_document::block_parser::parsers::math_equation::MathEquationParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::Value;

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_math_equation_block(
  test: &mut BlockTestCore,
  formula: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(formula) = formula {
    data.insert("formula".to_string(), Value::String(formula));
  }

  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: BlockType::MathEquation.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_math_equation_parser_with_formula() {
  let mut test = BlockTestCore::new();
  let parser = MathEquationParser;

  let formula = "E = mc^2".to_string();
  let block = create_math_equation_block(&mut test, Some(formula.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = format!("```math\n{}\n```", formula);
  assert_eq!(result.content, expected);
}

#[test]
fn test_math_equation_parser_empty_formula() {
  let mut test = BlockTestCore::new();
  let parser = MathEquationParser;

  let block = create_math_equation_block(&mut test, None, "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  let expected = "```math\n\n```";
  assert_eq!(result.content, expected);
}

#[test]
fn test_math_equation_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = MathEquationParser;

  let formula = "E = mc^2".to_string();
  let block = create_math_equation_block(&mut test, Some(formula.clone()), "");

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, formula);
}
