use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the simple table block.
///
/// Simple table block:
/// - A container that holds multiple simple table row blocks
pub struct SimpleTableParser;

impl BlockParser for SimpleTableParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    match context.format {
      OutputFormat::PlainText => {
        // For plain text, just use the default container behavior
        Ok(ParseResult::container("".to_string()))
      },
      OutputFormat::Markdown => {
        // For markdown, we need to handle the table separator row
        if block.children.is_empty() {
          return Ok(ParseResult::new("".to_string()));
        }

        if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
          let child_context = context.with_depth(context.depth + 1);
          let mut row_contents: Vec<String> = Vec::new();

          for (row_index, child_id) in child_ids.iter().enumerate() {
            if let Some(child_block) = context.document_data.blocks.get(child_id) {
              let row_content = context
                .parser
                .parse_block(child_block, &child_context)
                .unwrap_or_default();
              if row_index == 0 && !row_content.is_empty() {
                let num_columns = row_content.matches(" | ").count() + 1;
                row_contents.push(row_content);
                let separator =
                  format!("|{}|", "------|".repeat(num_columns).trim_end_matches('|'));
                row_contents.push(separator);
              } else {
                row_contents.push(row_content);
              }
            }
          }

          let result = row_contents.join("\n");
          return Ok(ParseResult::new(result));
        }

        Ok(ParseResult::new("".to_string()))
      },
    }
  }

  fn block_type(&self) -> &'static str {
    BlockType::SimpleTable.as_str()
  }
}
