use std::collections::HashMap;

use collab_document::block_parser::parsers::callout::CalloutParser;
use collab_document::block_parser::{BlockParser, DocumentParser, OutputFormat, ParseContext};
use collab_document::blocks::{Block, BlockType};
use serde_json::{Value, json};

use crate::blocks::block_test_core::{BlockTestCore, generate_id};

fn create_callout_block(
  test: &mut BlockTestCore,
  text: String,
  icon: Option<String>,
  parent_id: &str,
) -> Block {
  let mut data = HashMap::new();
  if let Some(icon) = icon {
    data.insert("icon".to_string(), Value::String(icon));
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
    ty: BlockType::Callout.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_callout_parser_with_custom_icon() {
  let mut test = BlockTestCore::new();
  let parser = CalloutParser;

  let block = create_callout_block(
    &mut test,
    "Hello AppFlowy".to_string(),
    Some("⚠️".to_string()),
    "",
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "> ⚠️ Hello AppFlowy");
}

#[test]
fn test_callout_parser_plain_text_format() {
  let mut test = BlockTestCore::new();
  let parser = CalloutParser;

  let block = create_callout_block(
    &mut test,
    "Hello AppFlowy".to_string(),
    Some("📝".to_string()),
    "",
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "📝 Hello AppFlowy");
}

#[test]
fn test_callout_parser_with_indent() {
  let mut test = BlockTestCore::new();
  let parser = CalloutParser;

  let block = create_callout_block(
    &mut test,
    "Hello AppFlowy".to_string(),
    Some("💡".to_string()),
    "",
  );

  let document_data = test.get_document_data();
  let document_parser = DocumentParser::with_default_parsers();
  let context =
    ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown).with_depth(2);
  let result = parser.parse(&block, &context).unwrap();

  assert_eq!(result.content, "    > 💡 Hello AppFlowy");
}
