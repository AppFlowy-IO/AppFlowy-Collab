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
    Ok(ParseResult::new(children_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Page.as_str()
  }

  // Custom parse_children implementation that keeps children at the same depth level (root level)
  fn parse_children(&self, block: &Block, context: &ParseContext) -> String {
    if block.children.is_empty() {
      return "".to_string();
    }

    if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
      // Use the same context (same depth) instead of incrementing depth
      let child_context = context;

      let result = child_ids
        .iter()
        .filter_map(|child_id| context.document_data.blocks.get(child_id))
        .filter_map(|child_block| context.parser.parse_block(child_block, child_context).ok())
        .filter(|child_content| !child_content.is_empty())
        .collect::<Vec<String>>()
        .join("\n");

      return result;
    }

    "".to_string()
  }
}
