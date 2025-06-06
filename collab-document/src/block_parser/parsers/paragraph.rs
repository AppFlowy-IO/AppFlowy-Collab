use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, ParseContext, ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the paragraph block.
///
/// Paragraph block data:
///   - delta: delta
pub struct ParagraphParser;

impl BlockParser for ParagraphParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let children_content = self.parse_children(block, context);

    let mut result = content;
    if !children_content.is_empty() {
      if !result.is_empty() {
        result.push('\n');
      }
      result.push_str(&children_content);
    }

    Ok(ParseResult::new(result))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Paragraph.as_str()
  }
}
