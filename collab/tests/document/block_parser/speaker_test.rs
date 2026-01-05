use crate::blocks::block_test_core::{BlockTestCore, generate_id};
use collab::document::block_parser::{DocumentParser, OutputFormat, ParseContext};
use collab::document::blocks::{Block, BlockType};
use collab::document::importer::define::{SPEAKER_ID_FIELD, SPEAKER_INFO_MAP_FIELD};
use serde_json::json;
use std::collections::HashMap;

fn create_ai_meeting_block(test: &mut BlockTestCore, speaker_info_map_json: &str) -> Block {
  let mut data = HashMap::new();
  data.insert(
    SPEAKER_INFO_MAP_FIELD.to_string(),
    json!(speaker_info_map_json),
  );

  let block = Block {
    id: generate_id(),
    ty: BlockType::AiMeeting.as_str().to_string(),
    parent: test.get_page().id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_ai_meeting_transcription_block(test: &mut BlockTestCore, parent_id: &str) -> Block {
  let data = HashMap::new();

  let block = Block {
    id: generate_id(),
    ty: BlockType::AiMeetingTranscription.as_str().to_string(),
    parent: parent_id.to_string(),
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, None).unwrap()
}

fn create_speaker_block(
  test: &mut BlockTestCore,
  parent_id: &str,
  speaker_id: &str,
  timestamp: f64,
  end_timestamp: f64,
  prev_id: Option<String>,
) -> Block {
  let mut data = HashMap::new();
  data.insert(SPEAKER_ID_FIELD.to_string(), json!(speaker_id));
  data.insert("timestamp".to_string(), json!(timestamp));
  data.insert("end_timestamp".to_string(), json!(end_timestamp));

  let block = Block {
    id: generate_id(),
    ty: BlockType::Speaker.as_str().to_string(),
    parent: parent_id.to_string(),
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };

  test.document.insert_block(block, prev_id).unwrap()
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

#[test]
fn test_speaker_parser_with_valid_speaker_map() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let speaker_info_map = json!({
    "A": {"name": "Lucas"},
    "B": {"name": "Richard"}
  });

  let ai_meeting = create_ai_meeting_block(&mut test, &speaker_info_map.to_string());
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  let speaker = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);
  create_text_block(
    &mut test,
    "Hello, this is the meeting content.",
    &speaker.id,
  );

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&speaker, &context).unwrap();

  assert_eq!(result, "Lucas: Hello, this is the meeting content.");
}

#[test]
fn test_speaker_parser_with_multiple_speakers() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let speaker_info_map = json!({
    "A": {"name": "Lucas"},
    "B": {"name": "Richard"}
  });

  let ai_meeting = create_ai_meeting_block(&mut test, &speaker_info_map.to_string());
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);

  let speaker_a = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);
  create_text_block(&mut test, "Hello from Lucas.", &speaker_a.id);

  let speaker_b = create_speaker_block(
    &mut test,
    &transcription.id,
    "B",
    10.5,
    20.0,
    Some(speaker_a.id.clone()),
  );
  create_text_block(&mut test, "Hello from Richard.", &speaker_b.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result_a = document_parser.parse_block(&speaker_a, &context).unwrap();
  let result_b = document_parser.parse_block(&speaker_b, &context).unwrap();

  assert_eq!(result_a, "Lucas: Hello from Lucas.");
  assert_eq!(result_b, "Richard: Hello from Richard.");
}

#[test]
fn test_speaker_parser_fallback_when_speaker_not_in_map() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let speaker_info_map = json!({
    "A": {"name": "Lucas"}
  });

  let ai_meeting = create_ai_meeting_block(&mut test, &speaker_info_map.to_string());
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  let speaker = create_speaker_block(&mut test, &transcription.id, "C", 0.0, 10.5, None);
  create_text_block(&mut test, "Hello from unknown speaker.", &speaker.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&speaker, &context).unwrap();

  assert_eq!(result, "Speaker C: Hello from unknown speaker.");
}

#[test]
fn test_speaker_parser_fallback_when_no_speaker_map() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let data = HashMap::new();
  let ai_meeting = Block {
    id: generate_id(),
    ty: BlockType::AiMeeting.as_str().to_string(),
    parent: test.get_page().id,
    children: generate_id(),
    external_id: None,
    external_type: None,
    data,
  };
  let ai_meeting = test.document.insert_block(ai_meeting, None).unwrap();

  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  let speaker = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);
  create_text_block(&mut test, "Hello from speaker.", &speaker.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&speaker, &context).unwrap();

  assert_eq!(result, "Speaker A: Hello from speaker.");
}

#[test]
fn test_speaker_parser_with_invalid_json() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let ai_meeting = create_ai_meeting_block(&mut test, "{ invalid json }");
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  let speaker = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);
  create_text_block(&mut test, "Hello from speaker.", &speaker.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&speaker, &context).unwrap();

  assert_eq!(result, "Speaker A: Hello from speaker.");
}

#[test]
fn test_speaker_parser_with_empty_children() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let speaker_info_map = json!({
    "A": {"name": "Lucas"}
  });

  let ai_meeting = create_ai_meeting_block(&mut test, &speaker_info_map.to_string());
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);
  let speaker = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser.parse_block(&speaker, &context).unwrap();

  assert_eq!(result, "Lucas: ");
}

#[test]
fn test_speaker_parser_in_full_meeting_context() {
  let mut test = BlockTestCore::new();
  let document_parser = DocumentParser::with_default_parsers();

  let speaker_info_map = json!({
    "A": {"name": "Lucas"},
    "B": {"name": "Richard"}
  });

  let ai_meeting = create_ai_meeting_block(&mut test, &speaker_info_map.to_string());
  let transcription = create_ai_meeting_transcription_block(&mut test, &ai_meeting.id);

  let speaker_a = create_speaker_block(&mut test, &transcription.id, "A", 0.0, 10.5, None);
  create_text_block(&mut test, "Welcome everyone to the meeting.", &speaker_a.id);

  let speaker_b = create_speaker_block(
    &mut test,
    &transcription.id,
    "B",
    10.5,
    25.0,
    Some(speaker_a.id),
  );
  create_text_block(
    &mut test,
    "Thank you. Let's begin with the agenda.",
    &speaker_b.id,
  );

  let speaker_a2 = create_speaker_block(
    &mut test,
    &transcription.id,
    "A",
    25.0,
    35.0,
    Some(speaker_b.id),
  );
  create_text_block(&mut test, "Great idea.", &speaker_a2.id);

  let document_data = test.get_document_data();
  let context = ParseContext::new(&document_data, &document_parser, OutputFormat::PlainText);

  let result = document_parser
    .parse_block(&transcription, &context)
    .unwrap();

  let expected = r#"Lucas: Welcome everyone to the meeting.
Richard: Thank you. Let's begin with the agenda.
Lucas: Great idea."#;

  assert_eq!(result, expected);
}
