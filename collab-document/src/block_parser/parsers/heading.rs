use serde_json::Value;

use crate::block_parser::{
  BlockParser, BlockParserRegistry, DefaultDocumentTextExtractor, DocumentParser,
  DocumentTextExtractor, OutputFormat, ParseContext, ParseResult,
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

    // the children of heading should be at the same level as the heading
    // so we need to use the same context for the children
    let child_context = context.with_depth(context.depth - 1);
    let children_content = self.parse_children(block, &child_context);

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

  // // Custom parse_children implementation that uses the registry to handle different child types
  // fn parse_children(&self, block: &Block, context: &ParseContext) -> String {
  //   if block.children.is_empty() {
  //     return "".to_string();
  //   }

  //   if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
  //     let child_context = context.with_depth(context.depth + 1);

  //     // Create a temporary DocumentParser with default parsers to handle children
  //     let document_parser = DocumentParser::with_default_parsers();

  //     let result = child_ids
  //       .iter()
  //       .filter_map(|child_id| context.document_data.blocks.get(child_id))
  //       .filter_map(|child_block| {
  //         document_parser
  //           .parse_block(child_block, &child_context)
  //           .ok()
  //       })
  //       .filter(|child_content| !child_content.is_empty())
  //       .collect::<Vec<String>>()
  //       .join("\n");

  //     return result;
  //   }

  //   "".to_string()
  // }
}
