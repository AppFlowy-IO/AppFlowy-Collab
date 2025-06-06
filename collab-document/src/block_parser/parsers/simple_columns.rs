use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple columns block.
///
/// Simple columns block:
/// - A container
pub struct SimpleColumnsParser;

impl BlockParser for SimpleColumnsParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple columns block is a container that holds multiple simple column blocks.
    // the children of simple columns are simple column blocks.
    Ok(ParseResult::container("".to_string()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleColumns.as_str()
  }
}
