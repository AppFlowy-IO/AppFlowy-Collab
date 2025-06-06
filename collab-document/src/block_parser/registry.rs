use crate::block_parser::{BlockParser, ParseContext, ParseResult};
use crate::blocks::Block;
use crate::error::DocumentError;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Clone)]
pub struct BlockParserRegistry {
  parsers: HashMap<String, Arc<dyn BlockParser + Send + Sync>>,
}

impl Debug for BlockParserRegistry {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BlockParserRegistry")
      .field("parsers", &self.parsers.keys().collect::<Vec<_>>())
      .finish()
  }
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

impl Default for BlockParserRegistry {
  fn default() -> Self {
    Self::new()
  }
}
