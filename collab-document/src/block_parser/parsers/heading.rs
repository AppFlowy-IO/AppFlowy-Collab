use serde_json::Value;

use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the heading block.
///
/// Heading block data:
///   level: int,
///   delta: delta,
pub struct HeadingParser;

const MAX_LEVEL: usize = 6;
const MIN_LEVEL: usize = 1;

// do not change the key value, it comes from the flutter code.
const LEVEL_KEY: &str = "level";

impl BlockParser for HeadingParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let level = block
      .data
      .get(LEVEL_KEY)
      .and_then(|v| match v {
        Value::Number(n) => n.as_u64().map(|n| n as usize),
        Value::String(s) => s.parse::<usize>().ok(),
        _ => None,
      })
      .unwrap_or(1)
      .clamp(MIN_LEVEL, MAX_LEVEL);

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        format!("{} {}", "#".repeat(level), content)
      },
      OutputFormat::PlainText => content,
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
    BlockType::Heading.as_str()
  }
}
