use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the quote list block.
///
/// Quote list block is typically used for blockquotes or quoted text.
pub struct QuoteListParser;

impl BlockParser for QuoteListParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        format!("{}> {}", indent, content)
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}{}", indent, content)
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
    BlockType::Quote.as_str()
  }
}
