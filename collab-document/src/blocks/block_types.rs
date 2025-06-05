use std::str::FromStr;

use serde::{Deserialize, Serialize};

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
}

impl BlockType {
  pub fn as_str(&self) -> &'static str {
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
    }
  }
}

impl FromStr for BlockType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "page" => Ok(BlockType::Page),
      "paragraph" => Ok(BlockType::Paragraph),
      "todo_list" => Ok(BlockType::TodoList),
      "bulleted_list" => Ok(BlockType::BulletedList),
      "numbered_list" => Ok(BlockType::NumberedList),
      "quote" => Ok(BlockType::Quote),
      "heading" => Ok(BlockType::Heading),
      "image" => Ok(BlockType::Image),
      "divider" => Ok(BlockType::Divider),
      "multi_image" => Ok(BlockType::MultiImage),
      "grid" => Ok(BlockType::Grid),
      "board" => Ok(BlockType::Board),
      "calendar" => Ok(BlockType::Calendar),
      "callout" => Ok(BlockType::Callout),
      "math_equation" => Ok(BlockType::MathEquation),
      "code" => Ok(BlockType::Code),
      "ai_writer" => Ok(BlockType::AiWriter),
      "toggle_list" => Ok(BlockType::ToggleList),
      "outline" => Ok(BlockType::Outline),
      "link_preview" => Ok(BlockType::LinkPreview),
      "video" => Ok(BlockType::Video),
      "file" => Ok(BlockType::File),
      "sub_page" => Ok(BlockType::SubPage),
      "errorBlockComponentBuilderKey" => Ok(BlockType::Error),
      "simple_table" => Ok(BlockType::SimpleTable),
      "simple_table_row" => Ok(BlockType::SimpleTableRow),
      "simple_table_cell" => Ok(BlockType::SimpleTableCell),
      "simple_columns" => Ok(BlockType::SimpleColumns),
      "simple_column" => Ok(BlockType::SimpleColumn),
      _ => Err(format!("Unknown block type: {}", s)),
    }
  }
}
