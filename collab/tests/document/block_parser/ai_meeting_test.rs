use crate::blocks::block_test_core::{BlockTestCore, generate_id};
use collab::document::block_parser::{DocumentParser, OutputFormat, ParseContext};
use collab::document::blocks::{Block, BlockType};
use serde_json::json;
use std::collections::HashMap;

fn create_ai_meeting_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  create_block(test, BlockType::AiMeeting, parent_id)
}

fn create_ai_meeting_summary_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  create_block(test, BlockType::AiMeetingSummary, parent_id)
}

fn create_ai_meeting_notes_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  create_block(test, BlockType::AiMeetingNotes, parent_id)
}

fn create_ai_meeting_transcription_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  create_block(test, BlockType::AiMeetingTranscription, parent_id)
}

fn create_block(test: &mut BlockTestCore, ty: BlockType, parent_id: &str) -> Block {
  let data = HashMap::new();
  let actual_parent_id = if parent_id.is_empty() {
    test.get_page().id
  } else {
    parent_id.to_string()
  };

  let block = Block {
    id: generate_id(),
    ty: ty.as_str().to_string(),
    parent: actual_parent_id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_text_block(test: &mut BlockTestCore, text: &str, parent_id: &str) -> Block {
  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);
  let data = HashMap::new();

  let block = Block {
    id: generate_id(),
    ty: BlockType::Paragraph.as_str().to_string(),
    parent: parent_id.to_string(),
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };
  test.document.insert_block(block, None).unwrap()
}

fn create_heading_block(
  test: &mut BlockTestCore,
  text: &str,
  level: u32,
  parent_id: &str,
) -> Block {
  let delta = json!([{ "insert": text }]).to_string();
  let external_id = test.create_text(delta);
  let mut data = HashMap::new();
  data.insert("level".to_string(), json!(level));

  let block = Block {
    id: generate_id(),
    ty: BlockType::Heading.as_str().to_string(),
    parent: parent_id.to_string(),
    children: generate_id(),
    external_id: Some(external_id),
    external_type: Some("text".to_string()),
    data,
  };
  test.document.insert_block(block, None).unwrap()
}

#[test]
fn test_ai_meeting_parser() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let ai_meeting = create_ai_meeting_block(&mut test, "");

  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Transcription content.", &transcription.id);

  let notes = create_ai_meeting_notes_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Notes content.", &notes.id);

  let summary = create_ai_meeting_summary_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Summary content.", &summary.id);
  create_heading_block(&mut test, "Overview", 4, &summary.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::Markdown);

  let result = document_parser.parse_block(&ai_meeting, &context).unwrap();

  let expected = r#"#### Overview
Summary content.
Notes content.
Transcription content."#;

  assert_eq!(result, expected);
}

#[test]
fn test_ai_meeting_parser_plain_text() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let ai_meeting = create_ai_meeting_block(&mut test, "");

  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Transcription content.", &transcription.id);

  let notes = create_ai_meeting_notes_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Notes content.", &notes.id);

  let summary = create_ai_meeting_summary_block(&mut test, &ai_meeting.id);
  create_text_block(&mut test, "Summary content.", &summary.id);
  create_heading_block(&mut test, "Overview", 4, &summary.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&ai_meeting, &context).unwrap();

  let expected = r#"Overview
Summary content.
Notes content.
Transcription content."#;

  assert_eq!(result, expected);
}
