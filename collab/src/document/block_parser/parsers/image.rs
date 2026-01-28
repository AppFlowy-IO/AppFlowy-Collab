use super::super::{BlockParser, ParseContext, ParseResult};
use crate::document::blocks::{Block, BlockType};
use crate::error::CollabError;

/// Parse the image block.
///
/// Image block data:
///   - url: the image URL
pub struct ImageParser;

// do not change the key value, it comes from the flutter code.
const URL_KEY: &str = "url";

impl BlockParser for ImageParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, CollabError> {
    // Extract the URL from block data
    let url = block
      .data
      .get(URL_KEY)
      .and_then(|v| v.as_str())
      .unwrap_or("");

    let formatted_content = match context.format {
      crate::document::OutputFormat::Markdown => {
        if url.is_empty() {
          "![Image]()".to_string()
        } else {
          format!("![Image]({})", url)
        }
      },
      crate::document::OutputFormat::PlainText => {
        if let Some(resolver) = context.plain_text_resolver() {
          if let Some(content) = resolver.resolve_block_text(block, context) {
            content
          } else {
            url.to_string()
          }
        } else {
          url.to_string()
        }
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
    BlockType::Image.as_str()
  }
}
