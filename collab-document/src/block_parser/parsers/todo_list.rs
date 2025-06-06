use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the todo list block.
///
/// Todo list block data:
///   checked: bool
pub struct TodoListParser;

// do not change the key value, it comes from the flutter code.
const CHECKED_KEY: &str = "checked";

impl BlockParser for TodoListParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let is_checked = block
      .data
      .get(CHECKED_KEY)
      .and_then(|v| v.as_bool())
      .unwrap_or(false);

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        let checkbox = if is_checked { "[x]" } else { "[ ]" };
        format!("{}- {} {}", indent, checkbox, content)
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
    BlockType::TodoList.as_str()
  }
}
