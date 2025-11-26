use super::super::{BlockParser, ParseContext, ParseResult};
use crate::document::blocks::{Block, BlockType};
use crate::error::CollabError;

pub struct AiMeetingTranscriptionParser;

impl BlockParser for AiMeetingTranscriptionParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, CollabError> {
    Ok(ParseResult::container("".to_string()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::AiMeetingTranscription.as_str()
  }
}
