use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple table row block.
///
/// Simple table row block:
/// - A container that holds multiple simple table cell blocks
pub struct SimpleTableRowParser;

impl BlockParser for SimpleTableRowParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple table row block is a container that holds multiple simple table cell blocks.
    // the children of simple table row are simple table cell blocks.
    Ok(ParseResult::container(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTableRow.as_str()
  }
}
