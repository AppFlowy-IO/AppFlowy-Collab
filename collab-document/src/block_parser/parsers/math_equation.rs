use serde_json::Value;

use crate::block_parser::{BlockParser, OutputFormat, ParseContext, ParseResult};
use crate::blocks::{Block, BlockType};
use crate::error::DocumentError;

/// Parse the math equation block.
///
/// Math equation block data:
///   formula: string
pub struct MathEquationParser;

// do not change the key value, it comes from the flutter code.
const FORMULA_KEY: &str = "formula";

impl BlockParser for MathEquationParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError> {
    let formula = block
      .data
      .get(FORMULA_KEY)
      .and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        _ => None,
      })
      .unwrap_or_default();

    let formatted_content = match context.format {
      OutputFormat::Markdown => {
        let indent = context.get_indent();
        if formula.is_empty() {
          format!("{}```math\n{}\n{}```", indent, indent, indent)
        } else {
          format!("{}```math\n{}{}\n{}```", indent, indent, formula, indent)
        }
      },
      OutputFormat::PlainText => {
        let indent = context.get_indent();
        format!("{}{}", indent, formula)
      },
    };

    Ok(ParseResult::new(formatted_content))
  }

  fn block_type(&self) -> &'static str {
    BlockType::MathEquation.as_str()
  }
}
