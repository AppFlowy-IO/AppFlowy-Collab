use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple table cell block.
///
/// Simple table cell block:
/// - A container that holds content (multiple blocks like paragraphs, headings, etc.)
pub struct SimpleTableCellParser;

impl BlockParser for SimpleTableCellParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // simple table cell block is a container that holds content.
    // Return empty content but signal that this block has children.
    Ok(ParseResult::container(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTableCell.as_str()
  }
}
