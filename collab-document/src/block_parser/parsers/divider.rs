use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the divider block.
///
/// Divider blocks have no data and no children.
pub struct DividerParser;

impl BlockParser for DividerParser {
  fn parse(&self, _block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        format!("{}---", indent)
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}---", indent)
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Divider.as_str()
  }
}
