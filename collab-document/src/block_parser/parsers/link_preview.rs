use serde_json::Value;

use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the link preview block.
///
/// Link preview block data:
///   url: string
pub struct LinkPreviewParser;

// do not change the key value, it comes from the flutter code.
const URL_KEY: &str = "url";

impl BlockParser for LinkPreviewParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let url = block
      .data
      .get(URL_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_default();

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        if url.is_empty() {
          "".to_string()
        } else {
          format!("{}[{}]({})", indent, url, url)
        }
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        if url.is_empty() {
          "".to_string()
        } else {
          format!("{}{}", indent, url)
        }
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::LinkPreview.as_str()
  }
}
