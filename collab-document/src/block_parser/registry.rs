use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::Block;
use crate::error::DocumentError;
use std::collections::HashMap;
use std::sync::Arc;

pub struct BlockParserRegistry {
  parsers: HashMap<String, Arc<dyn BlockParser + Send + Sync>>,
}

impl BlockParserRegistry {
  pub fn new() -> Self {
    Self {
      parsers: HashMap::new(),
    }
  }

  pub fn register(&mut self, parser: Arc<dyn BlockParser + Send + Sync>) -> &mut Self {
    let block_type = parser.block_type().to_string();
    self.parsers.insert(block_type, parser);
    self
  }

  pub fn unregister(&mut self, block_type: &str) -> Option<Arc<dyn BlockParser + Send + Sync>> {
    self.parsers.remove(block_type)
  }

  pub fn get_parser(&self, block_type: &str) -> Option<&Arc<dyn BlockParser + Send + Sync>> {
    self.parsers.get(block_type)
  }

  pub fn parse_block(
    &self,
    block: &Block,
    context: &ParseContext,
  ) -> Result<ParseResult, DocumentError> {
    if let Some(parser) = self.get_parser(&block.ty) {
      parser.parse(block, context)
    } else {
      Ok(ParseResult::empty())
    }
  }
}
