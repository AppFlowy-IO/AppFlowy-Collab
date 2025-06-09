use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::error::DocumentError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockType {
  Page,
  Paragraph,
  TodoList,
  BulletedList,
  NumberedList,
  Quote,
  Heading,
  Image,
  Divider,
  MultiImage,
  Grid,
  Board,
  Calendar,
  Callout,
  MathEquation,
  Code,
  AiWriter,
  ToggleList,
  Outline,
  LinkPreview,
  Video,
  File,
  SubPage,
  Error,
  SimpleTable,
  SimpleTableRow,
  SimpleTableCell,
  SimpleColumns,
  SimpleColumn,
  Custom(String),

  // Legacy types
  Table,
  TableCell,
}

impl BlockType {
  pub fn as_str(&self) -> &str {
    match self {
      BlockType::Page => "page",
      BlockType::Paragraph => "paragraph",
      BlockType::TodoList => "todo_list",
      BlockType::BulletedList => "bulleted_list",
      BlockType::NumberedList => "numbered_list",
      BlockType::Quote => "quote",
      BlockType::Heading => "heading",
      BlockType::Image => "image",
      BlockType::Divider => "divider",
      BlockType::MultiImage => "multi_image",
      BlockType::Grid => "grid",
      BlockType::Board => "board",
      BlockType::Calendar => "calendar",
      BlockType::Callout => "callout",
      BlockType::MathEquation => "math_equation",
      BlockType::Code => "code",
      BlockType::AiWriter => "ai_writer",
      BlockType::ToggleList => "toggle_list",
      BlockType::Outline => "outline",
      BlockType::LinkPreview => "link_preview",
      BlockType::Video => "video",
      BlockType::File => "file",
      BlockType::SubPage => "sub_page",
      BlockType::Error => "errorBlockComponentBuilderKey",
      BlockType::SimpleTable => "simple_table",
      BlockType::SimpleTableRow => "simple_table_row",
      BlockType::SimpleTableCell => "simple_table_cell",
      BlockType::SimpleColumns => "simple_columns",
      BlockType::SimpleColumn => "simple_column",
      BlockType::Table => "table",
      BlockType::TableCell => "table/cell",
      BlockType::Custom(s) => s,
    }
  }

  pub fn from_block_ty(s: &str) -> Self {
    match s {
      "page" => BlockType::Page,
      "paragraph" => BlockType::Paragraph,
      "todo_list" => BlockType::TodoList,
      "bulleted_list" => BlockType::BulletedList,
      "numbered_list" => BlockType::NumberedList,
      "quote" => BlockType::Quote,
      "heading" => BlockType::Heading,
      "image" => BlockType::Image,
      "divider" => BlockType::Divider,
      "multi_image" => BlockType::MultiImage,
      "grid" => BlockType::Grid,
      "board" => BlockType::Board,
      "calendar" => BlockType::Calendar,
      "callout" => BlockType::Callout,
      "math_equation" => BlockType::MathEquation,
      "code" => BlockType::Code,
      "ai_writer" => BlockType::AiWriter,
      "toggle_list" => BlockType::ToggleList,
      "outline" => BlockType::Outline,
      "link_preview" => BlockType::LinkPreview,
      "video" => BlockType::Video,
      "file" => BlockType::File,
      "sub_page" => BlockType::SubPage,
      "errorBlockComponentBuilderKey" => BlockType::Error,
      "simple_table" => BlockType::SimpleTable,
      "simple_table_row" => BlockType::SimpleTableRow,
      "simple_table_cell" => BlockType::SimpleTableCell,
      "simple_columns" => BlockType::SimpleColumns,
      "simple_column" => BlockType::SimpleColumn,
      "table" => BlockType::Table,
      "table/cell" => BlockType::TableCell,
      _ => BlockType::Custom(s.to_string()),
    }
  }
}

impl FromStr for BlockType {
  type Err = DocumentError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::from_block_ty(s))
  }
}

// Implement AsRef<str> for ContentType
impl AsRef<str> for BlockType {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl Display for BlockType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_str())
  }
}
