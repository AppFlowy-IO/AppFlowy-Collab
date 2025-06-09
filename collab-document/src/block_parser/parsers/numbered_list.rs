use serde_json::Value;

use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the numbered list block.
///
/// Numbered list block data:
///   number: string,
///   delta: delta,
pub struct NumberedListParser;

// do not change the key value, it comes from the flutter code.
const NUMBER_KEY: &str = "number";

impl BlockParser for NumberedListParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let number = block
      .data
      .get(NUMBER_KEY)
      .and_then(|v| match v {
        Value::Number(n) => n.as_u64().map(|n| n as usize),
        Value::String(s) => s.parse::<usize>().ok(),
        _ => None,
      })
      .unwrap_or_else(|| context.list_number.unwrap_or(1));

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        format!("{}{}. {}", indent, number, content)
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}{}. {}", indent, number, content)
      },
    };

    let list_context = context.with_list_context(Some(number + 1));
    let children_content = self.parse_children(block, &list_context);

    let mut result = formatted_content;
    if !children_content.is_empty() {
      result.push('\n');
      result.push_str(&children_content);
    }

    Ok(ParseResult::new(result))
  }

  fn block_type(&self) -> &'static str {
    BlockType::NumberedList.as_str()
  }
}
