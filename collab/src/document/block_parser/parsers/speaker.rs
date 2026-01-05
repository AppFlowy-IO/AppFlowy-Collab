use super::super::{BlockParser, ParseContext, ParseResult};
use crate::document::blocks::{Block, BlockType};
use crate::document::importer::define::{SPEAKER_ID_FIELD, SPEAKER_INFO_MAP_FIELD};
use crate::error::CollabError;

/// Parse the speaker block.
///
/// Speaker block data:
///   - speaker_id: String (identifier for the speaker, defaults to 'A')
///   - timestamp: f64 (start timestamp, defaults to 0.0)
///   - end_timestamp: f64 (end timestamp, defaults to 0.0)
pub struct SpeakerParser;

impl SpeakerParser {
  fn get_speaker_name(&self, block: &Block, context: &ParseContext) -> Option<String> {
    let speaker_id = block.data.get(SPEAKER_ID_FIELD)?.as_str()?;

    let transcription_block = context.document_data.blocks.get(&block.parent)?;
    let ai_meeting_block = context
      .document_data
      .blocks
      .get(&transcription_block.parent)?;

    if ai_meeting_block.ty != BlockType::AiMeeting.as_str() {
      return None;
    }

    let speaker_info_map_str = ai_meeting_block
      .data
      .get(SPEAKER_INFO_MAP_FIELD)?
      .as_str()?;

    serde_json::from_str::<serde_json::Value>(speaker_info_map_str)
      .ok()?
      .get(speaker_id)?
      .get("name")?
      .as_str()
      .map(|s| s.to_string())
  }
}

impl BlockParser for SpeakerParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, CollabError> {
    let speaker_name = self.get_speaker_name(block, context);

    let prefix = if let Some(name) = speaker_name {
      format!("{}: ", name)
    } else {
      if let Some(speaker_id) = block.data.get(SPEAKER_ID_FIELD).and_then(|v| v.as_str()) {
        format!("Speaker {}: ", speaker_id)
      } else {
        "Unknown Speaker: ".to_string()
      }
    };

    let children_content = self.parse_children(block, context);

    let mut result = prefix;
    if !children_content.is_empty() {
      result.push_str(children_content.trim_end_matches('\n'));
    }

    Ok(ParseResult::new(result))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Speaker.as_str()
  }
}
