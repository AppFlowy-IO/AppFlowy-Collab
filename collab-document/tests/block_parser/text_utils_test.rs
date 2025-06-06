use collab::preclude::{Any, Attrs};
use collab_document::block_parser::*;
use std::sync::Arc;

#[test]
fn test_format_text_with_attributes() {
  let mut attrs = Attrs::new();
  attrs.insert(Arc::from("bold"), Any::Bool(true));

  let result = format_text_with_attributes("test", &attrs);
  assert_eq!(result, "**test**");

  let mut attrs = Attrs::new();
  attrs.insert(Arc::from("italic"), Any::Bool(true));
  let result = format_text_with_attributes("test", &attrs);
  assert_eq!(result, "*test*");

  let mut attrs = Attrs::new();
  attrs.insert(Arc::from("strikethrough"), Any::Bool(true));
  let result = format_text_with_attributes("test", &attrs);
  assert_eq!(result, "~~test~~");

  let mut attrs = Attrs::new();
  attrs.insert(
    Arc::from("href"),
    Any::String(Arc::from("https://appflowy.io")),
  );
  let result = format_text_with_attributes("test", &attrs);
  assert_eq!(result, "[test](https://appflowy.io)");

  let mut attrs = Attrs::new();
  attrs.insert(Arc::from("code"), Any::Bool(true));
  let result = format_text_with_attributes("test", &attrs);
  assert_eq!(result, "`test`");
}

#[test]
fn test_text_extractor_basic() {
  let delta_json = r#"[{"insert": "Hello AppFlowy"}]"#;
  let result = DefaultDocumentTextExtractor
    .extract_plain_text_from_delta_with_context(delta_json, None)
    .unwrap();
  assert_eq!(result, "Hello AppFlowy");
}

#[test]
fn test_text_extractor_delta_parsing() {
  let delta_json = r#"[
    {"insert": "Hello", "attributes": {"bold": true}},
    {"insert": " "},
    {"insert": "AppFlowy", "attributes": {"italic": true}}
  ]"#;

  let plain_result = DefaultDocumentTextExtractor
    .extract_plain_text_from_delta_with_context(delta_json, None)
    .unwrap();
  assert_eq!(plain_result, "Hello AppFlowy");

  let markdown_result = DefaultDocumentTextExtractor
    .extract_markdown_text_from_delta_with_context(delta_json, None)
    .unwrap();
  assert_eq!(markdown_result, "**Hello** *AppFlowy*");
}

#[test]
fn test_text_extractor_mentions() {
  // Test delta with mentions - mentions should be filtered out in plain text
  let delta_json = r#"[
    {"insert": "Mention a page "},
    {"insert": "$", "attributes": {"mention": "@page_id"}}
  ]"#;

  let plain_result = DefaultDocumentTextExtractor
    .extract_plain_text_from_delta_with_context(delta_json, None)
    .unwrap();
  assert_eq!(plain_result, "Mention a page $");
}
