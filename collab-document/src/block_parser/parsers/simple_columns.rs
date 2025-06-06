use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple columns block.
///
/// Simple columns block:
/// - A container
pub struct SimpleColumnsParser;

impl BlockParser for SimpleColumnsParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple columns block is a container that holds multiple simple column blocks.
    // the children of simple columns are simple column blocks.
    // Return empty content but signal that this block has children.
    // The DocumentParser will handle parsing the children using the appropriate parsers.
    Ok(ParseResult::container(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleColumns.as_str()
  }
}
