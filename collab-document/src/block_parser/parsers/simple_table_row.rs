use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
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
      return Ok(ParseResult::new("".to_string()));
    }

    if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
      let child_context = context.with_depth(context.depth + 1);

      let cell_contents: Vec<String> = child_ids
        .iter()
        .filter_map(|child_id| context.document_data.blocks.get(child_id))
        .map(|child_block| {
          context
            .parser
            .parse_block(child_block, &child_context)
            .unwrap_or_default()
        })
        .collect();

      let result = match context.format {
        OutputFormat::PlainText => cell_contents.join("\t"),
        OutputFormat::Markdown => {
          format!("| {} |", cell_contents.join(" | "))
        },
      };

      return Ok(ParseResult::new(result));
    }

    Ok(ParseResult::new("".to_string()))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTableRow.as_str()
  }
}
