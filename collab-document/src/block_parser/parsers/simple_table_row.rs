use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple table row block.
///
/// Simple table row block:
/// - A container that holds multiple simple table cell blocks
pub struct SimpleTableRowParser;

impl BlockParser for SimpleTableRowParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    if block.children.is_empty() {
      return Ok(ParseResult::new(String::new()));
    }

    if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
      let child_context = context.with_depth(context.depth + 1);

      let mut cell_contents: Vec<String> = child_ids
        .iter()
        .filter_map(|child_id| context.document_data.blocks.get(child_id))
        .map(|child_block| {
          context
            .parser
            .parse_block(child_block, &child_context)
            .unwrap_or_default()
        })
        .collect();

      // Trim trailing empty cells
      while let Some(last) = cell_contents.last() {
        if last.is_empty() {
          cell_contents.pop();
        } else {
          break;
        }
      }

      let result = cell_contents.join("\t"); // Use tabs to separate cells

      return Ok(ParseResult::new(result));
    }

    Ok(ParseResult::new(String::new()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTableRow.as_str()
  }
}
