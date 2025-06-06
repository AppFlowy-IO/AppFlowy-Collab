use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple table block.
///
/// Simple table block:
/// - A container that holds multiple simple table row blocks
pub struct SimpleTableParser;

impl BlockParser for SimpleTableParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple table block is a container that holds multiple simple table row blocks.
    // the children of simple table are simple table row blocks.
    Ok(ParseResult::container(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTable.as_str()
  }
}
