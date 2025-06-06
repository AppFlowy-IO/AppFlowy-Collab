use serde_json::Value;

use crate::block_parser::{
  BlockParser, DefaultDocumentTextExtractor, DocumentTextExtractor, OutputFormat, ParseContext,
  ParseResult,
};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the code block.
///
/// Code block data:
///   language: string
///   delta: delta
pub struct CodeBlockParser;

// do not change the key value, it comes from the flutter code.
const LANGUAGE_KEY: &str = "language";

impl BlockParser for CodeBlockParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let text_extractor = DefaultDocumentTextExtractor;
    let content = text_extractor.extract_text_from_block(block, context)?;

    let language = block
      .data
      .get(LANGUAGE_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_default();

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        format!("{}```{}\n{}\n{}```", indent, language, content, indent)
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}{}", indent, content)
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::Code.as_str()
  }
}
