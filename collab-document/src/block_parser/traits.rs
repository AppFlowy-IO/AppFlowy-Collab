use std::fmt::Debug;

use crate::{
  block_parser::DocumentParser,
  blocks::{Block, DocumentData},
  error::DocumentError,
};
use collab::preclude::Attrs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
  PlainText,
  Markdown,
}

#[derive(Debug, Clone)]
pub struct ParseContext<'a> {
  pub document_data: &'a DocumentData,
  pub parser: &'a DocumentParser,
  pub format: OutputFormat,
  pub depth: usize,
  // use to control the indentation of the list
  // - line 1
  //    - line 2
  //        - line 3
  pub in_list: bool,
  // use to keep the previous list number
  pub list_number: Option<usize>,
  pub parent_type: Option<String>,
}

impl<'a> ParseContext<'a> {
  pub fn new(
    document_data: &'a DocumentData,
    parser: &'a DocumentParser,
    format: OutputFormat,
  ) -> Self {
    Self {
      document_data,
      parser,
      format,
      depth: 0,
      in_list: false,
      list_number: None,
      parent_type: None,
    }
  }

  pub fn with_depth(&self, depth: usize) -> Self {
    Self {
      document_data: self.document_data,
      parser: self.parser,
      format: self.format,
      depth,
      in_list: self.in_list,
      list_number: self.list_number,
      parent_type: self.parent_type.clone(),
    }
  }

  pub fn with_list_context(&self, list_number: Option<usize>) -> Self {
    Self {
      document_data: self.document_data,
      parser: self.parser,
      format: self.format,
      depth: self.depth,
      in_list: true,
      list_number,
      parent_type: self.parent_type.clone(),
    }
  }

  pub fn with_parent_type(&self, parent_type: String) -> Self {
    Self {
      document_data: self.document_data,
      parser: self.parser,
      format: self.format,
      depth: self.depth,
      in_list: self.in_list,
      list_number: self.list_number,
      parent_type: Some(parent_type),
    }
  }

  pub fn get_indent(&self) -> String {
    match self.format {
      OutputFormat::PlainText => "  ".repeat(self.depth),
      OutputFormat::Markdown => "  ".repeat(self.depth),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
  pub content: String,

  // if the block is empty, we don't need to add a newline
  pub add_newline: bool,

  // if the block has children, we need to parse the children content
  pub is_container: bool,
}

impl ParseResult {
  pub fn new(content: String) -> Self {
    Self {
      content,
      add_newline: true,
      is_container: false,
    }
  }

  pub fn no_newline(content: String) -> Self {
    Self {
      content,
      add_newline: false,
      is_container: false,
    }
  }

  pub fn container(content: String) -> Self {
    Self {
      content,
      add_newline: true,
      is_container: true,
    }
  }

  pub fn empty() -> Self {
    Self {
      content: "".to_string(),
      add_newline: false,
      is_container: false,
    }
  }
}

pub trait BlockParser {
  fn parse(&self, block: &Block, context: &ParseContext) -> Result<ParseResult, DocumentError>;

  fn block_type(&self) -> &'static str;

  fn can_parse(&self, block_type: &str) -> bool {
    self.block_type() == block_type
  }

  // In most case, when customizing the parser, we don't need to override this function
  //  unless you need to parse the children content with different format
  //  or the children have special nesting structure, like the simple_table and columns
  fn parse_children(&self, block: &Block, context: &ParseContext) -> String {
    if block.children.is_empty() {
      return "".to_string();
    }

    if let Some(child_ids) = context.document_data.meta.children_map.get(&block.children) {
      let child_context = context.with_depth(context.depth + 1);

      let result = child_ids
        .iter()
        .filter_map(|child_id| context.document_data.blocks.get(child_id))
        .filter_map(|child_block| context.parser.parse_block(child_block, &child_context).ok())
        .filter(|child_result| !child_result.is_empty())
        .fold("".to_string(), |mut acc, child_result| {
          acc.push_str(&child_result);
          acc.push('\n');
          acc
        });

      return result;
    }

    "".to_string()
  }
}

pub trait DocumentParserDelegate: Debug {
  /// Delegate the text delta to the caller.
  ///
  /// For example, for the mentioned page, the caller should return the page name based on the mentioned page id.
  fn handle_text_delta(
    &self,
    _text: &str,
    _attributes: Option<&Attrs>,
    _context: &ParseContext,
  ) -> Option<String> {
    None
  }
}
