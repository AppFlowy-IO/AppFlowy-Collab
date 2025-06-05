use crate::{
  block_parser::{ParseContext, ParseResult},
  blocks::Block,
  error::DocumentError,
};

pub trait BlockParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError>;

  fn block_type(&self) -> &'static str;

  fn can_parse(&self, block_type: &str) -> bool {
    self.block_type() == block_type
  }

  fn parse_children(&self, block: &Block, context: &ParseContext) -> Result<String, DocumentError> {
    let child_ids = context
      .document_data
      .meta
      .children_map
      .get(&block.children)
      .ok_or(DocumentError::NoBlockChildrenFound)?;

    let child_context = context.with_depth(context.depth + 1);

    let result = child_ids
      .iter()
      .filter_map(|child_id| context.document_data.blocks.get(child_id))
      .filter_map(|child_block| self.parse(child_block, &child_context).ok())
      .filter(|child_result| !child_result.content.is_empty())
      .fold(String::new(), |mut acc, child_result| {
        acc.push_str(&child_result.content);
        if child_result.add_newline {
          acc.push('\n');
        }
        acc
      });

    Ok(result)
  }
}
