use serde_json::Value;

use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the subpage block.
///
/// Subpage block data:
///   viewId: string
pub struct SubpageParser;

// do not change the key value, it comes from the flutter code.
const VIEW_ID_KEY: &str = "viewId";

impl BlockParser for SubpageParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let view_id = block
      .data
      .get(VIEW_ID_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_default();

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        if view_id.is_empty() {
          format!("{}[Subpage]", indent)
        } else {
          format!("{}[Subpage]({})", indent, view_id)
        }
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        if view_id.is_empty() {
          "".to_string()
        } else {
          format!("{}{}", indent, view_id)
        }
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::SubPage.as_str()
  }
}
