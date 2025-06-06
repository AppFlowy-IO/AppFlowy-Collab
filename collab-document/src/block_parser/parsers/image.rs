use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the image block.
///
/// Image block data:
///   - url: the image URL
pub struct ImageParser;

// do not change the key value, it comes from the flutter code.
const URL_KEY: &str = "url";

impl BlockParser for ImageParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    // Extract the URL from block data
    let url = block
      .data
      .get(URL_KEY)
      .and_then(|v| v.as_str())
      .unwrap_or("");

    let formatted_content = match context.format {
      crate::block_parser::OutputFormat::Markdown => {
        if url.is_empty() {
          "![Image]()".to_string()
        } else {
          format!("![Image]({})", url)
        }
      },
      crate::block_parser::OutputFormat::PlainText => url.to_string(),
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
