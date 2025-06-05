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
const LEVEL_KEY: &str = "level";

impl BlockParser for HeadingParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    if content.is_empty() {
      return Ok(ParseResult::empty());
    }

    // Get heading level from block data (default to 1)
    let level = block
      .data
      .get(LEVEL_KEY)
      .and_then(|v| v.as_u64().map(|n| n as usize))
      .unwrap_or(1)
      .clamp(MIN_LEVEL, MAX_LEVEL);

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        format!("{} {}", "#".repeat(level), content)
      },
      OutputFormat::PlainText => content,
    };

    // Add any children content
    let children_content = self.parse_children(block, context)?;

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
