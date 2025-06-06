use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple column block.
///
/// Simple column block:
/// - A container
pub struct SimpleColumnParser;

impl BlockParser for SimpleColumnParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple column block is a container that holds content.
    // Return empty content but signal that this block has children.
    // The DocumentParser will handle parsing the children using the appropriate parsers.
    Ok(ParseResult::container(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleColumn.as_str()
  }
}
