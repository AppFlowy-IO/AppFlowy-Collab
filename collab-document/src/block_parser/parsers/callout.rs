use serde_json::Value;

use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the callout block.
///
/// Callout block data:
///   icon: string,
///   delta: delta,
pub struct CalloutParser;

// do not change the key value, it comes from the flutter code.
const ICON_KEY: &str = "icon";

impl BlockParser for CalloutParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let icon = block
      .data
      .get(ICON_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_else(|| "💡".to_string()); // Default icon if none provided

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        format!("{}> {} {}", indent, icon, content)
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}{} {}", indent, icon, content)
      },
    };

    let children_content = self.parse_children(block, context);

    let mut result = formatted_content;
    if !children_content.is_empty() {
      result.push('\n');
      result.push_str(&children_content);
    }

    Ok(ParseResult::new(result))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Callout.as_str()
  }
}
