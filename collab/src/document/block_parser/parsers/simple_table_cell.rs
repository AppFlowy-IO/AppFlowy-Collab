use super::super::{BlockParser, ParseContext, ParseResult};
use crate::document::blocks::{Block, BlockType};
use crate::document::error::DocumentError;

/// Parse the simple table cell block.
///
/// Simple table cell block:
/// - A container that holds content (multiple blocks like paragraphs, headings, etc.)
pub struct SimpleTableCellParser;

impl BlockParser for SimpleTableCellParser {
  fn parse(&self, _block: &Block, _context: &ParseContext) -> Result<ParseResult, DocumentError> {
    Ok(ParseResult::container("".to_string()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTableCell.as_str()
  }
}
