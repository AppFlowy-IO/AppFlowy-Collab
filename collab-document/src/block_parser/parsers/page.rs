use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the page block.
///
/// Page block data:
///   - children: blocks
pub struct PageParser;

impl BlockParser for PageParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let children_content = self.parse_children(block, context);
    Ok(ParseResult::container(children_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Page.as_str()
  }
}
