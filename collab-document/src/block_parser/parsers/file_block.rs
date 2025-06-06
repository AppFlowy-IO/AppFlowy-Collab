use serde_json::Value;

use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the file block.
///
/// File block data:
///   name: string,
///   url: string
pub struct FileBlockParser;

// do not change the key values, they come from the flutter code.
const NAME_KEY: &str = "name";
const URL_KEY: &str = "url";

impl BlockParser for FileBlockParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let name = block
      .data
      .get(NAME_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_default();

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
          format!("{}{}", indent, name)
        } else {
          format!("{}[{}]({})", indent, name, url)
        }
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        if url.is_empty() {
          format!("{}{}", indent, name)
        } else {
          format!("{}{}({})", indent, name, url)
        }
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::File.as_str()
  }
}
