use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the toggle list block.
///
/// Toggle list block data:
///   delta: delta
pub struct ToggleListParser;

impl BlockParser for ToggleListParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let children_content = self.parse_children(block, context);

    let result = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        if children_content.is_empty() {
          format!(
            "{}<details>\n{}<summary>{}</summary>\n{}</details>",
            indent, indent, content, indent
          )
        } else {
          format!(
            "{}<details>\n{}<summary>{}</summary>\n\n{}\n{}</details>",
            indent, indent, content, children_content, indent
          )
        }
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        let mut result = format!("{}{}", indent, content);
        if !children_content.is_empty() {
          result.push('\n');
          result.push_str(&children_content);
        }
        result
      },
    };

    Ok(ParseResult::new(result))
  }

  fn block_type(&self) -> &'static str {
    BlockType::ToggleList.as_str()
  }
}
