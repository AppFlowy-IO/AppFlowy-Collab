use crate::block_parser::{
  BlockParserRegistry, BulletedListParser, CalloutParser, CodeBlockParser, DividerParser,
  DocumentParserDelegate, FileBlockParser, HeadingParser, ImageParser, LinkPreviewParser,
  MathEquationParser, NumberedListParser, OutputFormat, PageParser, ParagraphParser, ParseContext,
  QuoteListParser, SimpleColumnParser, SimpleColumnsParser, SimpleTableCellParser,
  SimpleTableParser, SimpleTableRowParser, SubpageParser, TodoListParser, ToggleListParser,
};
use crate::blocks::{Block, DocumentData};
use crate::error::DocumentError;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DocumentParser {
  registry: BlockParserRegistry,

  /// Provide the delegate to handle special cases like mentions during parsing
  delegate: Option<Arc<dyn DocumentParserDelegate + Send + Sync>>,
}

impl DocumentParser {
  pub fn new() -> Self {
    Self {
      registry: BlockParserRegistry::new(),
      delegate: None,
    }
  }

  pub fn with_delegate(mut self, delegate: Arc<dyn DocumentParserDelegate + Send + Sync>) -> Self {
    self.delegate = Some(delegate);
    self
  }

  pub fn set_delegate(&mut self, delegate: Arc<dyn DocumentParserDelegate + Send + Sync>) {
    self.delegate = Some(delegate);
  }

  pub fn get_delegate(&self) -> Option<&Arc<dyn DocumentParserDelegate + Send + Sync>> {
    self.delegate.as_ref()
  }

  pub fn with_default_parsers() -> Self {
    let mut parser = Self::new();

    parser
      .registry
      .register(Arc::new(PageParser))
      .register(Arc::new(ParagraphParser))
      .register(Arc::new(HeadingParser))
      .register(Arc::new(NumberedListParser))
      .register(Arc::new(BulletedListParser))
      .register(Arc::new(TodoListParser))
      .register(Arc::new(QuoteListParser))
      .register(Arc::new(ToggleListParser))
      .register(Arc::new(ImageParser))
      .register(Arc::new(CalloutParser))
      .register(Arc::new(CodeBlockParser))
      .register(Arc::new(DividerParser))
      .register(Arc::new(FileBlockParser))
      .register(Arc::new(LinkPreviewParser))
      .register(Arc::new(MathEquationParser))
      .register(Arc::new(SimpleColumnsParser))
      .register(Arc::new(SimpleColumnParser))
      .register(Arc::new(SimpleTableParser))
      .register(Arc::new(SimpleTableRowParser))
      .register(Arc::new(SimpleTableCellParser))
      .register(Arc::new(SubpageParser));

    parser
  }

  pub fn parse_document(
    &self,
    document_data: &DocumentData,
    format: OutputFormat,
  ) -> Result<String, DocumentError> {
    // find the page block first
    let page_block = document_data
      .blocks
      .get(&document_data.page_id)
      .ok_or(DocumentError::PageBlockNotFound)?;

    let context = ParseContext::new(document_data, self, format);
    self.parse_block(page_block, &context)
  }

  pub fn parse_block(
    &self,
    block: &Block,
    context: &ParseContext,
  ) -> Result<String, DocumentError> {
    let result = self.registry.parse_block(block, context)?;

    if result.is_container {
      let mut content = result.content;

      if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
        let child_context = context.with_depth(context.depth + 1);
        let children_content = self.parse_children(child_ids, &child_context)?;

        if !children_content.is_empty() {
          if !content.is_empty() && result.add_newline {
            content.push('\n');
          }
          content.push_str(&children_content);
        }
      }

      Ok(content)
    } else {
      Ok(result.content)
    }
  }

  fn parse_children(
    &self,
    child_ids: &[String],
    context: &ParseContext,
  ) -> Result<String, DocumentError> {
    let mut result = "".to_string();

    for child_id in child_ids {
      if let Some(child_block) = context.document_data.blocks.get(child_id) {
        let child_content = self.parse_block(child_block, context)?;
        if !child_content.is_empty() {
          if !result.is_empty() {
            result.push('\n');
          }
          result.push_str(&child_content);
        }
      }
    }

    Ok(result)
  }
}

impl Default for DocumentParser {
  fn default() -> Self {
    Self::with_default_parsers()
  }
}
