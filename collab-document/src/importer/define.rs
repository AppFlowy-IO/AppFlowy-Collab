use crate::error::DocumentError;
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq)]
pub enum BlockType {
  Page,
  Paragraph,
  Heading,
  Quote,
  TodoList,
  NumberedList,
  BulletedList,
  Image,
  LinkPreview,
  Code,
  MathEquation,
  Divider,
  Table,
  TableCell,
  Text,
  Custom(String),
}

impl BlockType {
  pub fn as_str(&self) -> &str {
    match self {
      BlockType::Page => "page",
      BlockType::Paragraph => "paragraph",
      BlockType::Heading => "heading",
      BlockType::Quote => "quote",
      BlockType::TodoList => "todo_list",
      BlockType::NumberedList => "numbered_list",
      BlockType::BulletedList => "bulleted_list",
      BlockType::Image => "image",
      BlockType::LinkPreview => "link_preview",
      BlockType::Code => "code",
      BlockType::MathEquation => "math_equation",
      BlockType::Divider => "divider",
      BlockType::Table => "table",
      BlockType::TableCell => "table/cell",
      BlockType::Text => "text",
      BlockType::Custom(s) => s,
    }
  }

  pub fn from_block_ty(s: &str) -> Self {
    match s {
      "page" => BlockType::Page,
      "paragraph" => BlockType::Paragraph,
      "heading" => BlockType::Heading,
      "quote" => BlockType::Quote,
      "todo_list" => BlockType::TodoList,
      "numbered_list" => BlockType::NumberedList,
      "bulleted_list" => BlockType::BulletedList,
      "image" => BlockType::Image,
      "link_preview" => BlockType::LinkPreview,
      "code" => BlockType::Code,
      "math_equation" => BlockType::MathEquation,
      "divider" => BlockType::Divider,
      "table" => BlockType::Table,
      "table/cell" => BlockType::TableCell,
      "text" => BlockType::Text,
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

pub const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "svg", "webp"];

// Data Attribute Keys
pub const DEFAULT_COL_WIDTH: i32 = 150;
pub const DEFAULT_ROW_HEIGHT: i32 = 37;

// Align
pub const ALIGN_LEFT: &str = "left";
pub const ALIGN_RIGHT: &str = "right";
pub const ALIGN_CENTER: &str = "center";

// Heading Keys
pub const LEVEL_FIELD: &str = "level";

// Code Keys
pub const LANGUAGE_FIELD: &str = "language";

// Link Keys
pub const URL_FIELD: &str = "url";

// Image Keys
pub const IMAGE_TYPE_FIELD: &str = "image_type";
pub const EXTERNAL_IMAGE_TYPE: i32 = 2;

// Math Equation Keys
pub const FORMULA_FIELD: &str = "formula";

// Delta Attribute Keys
pub const BOLD_ATTR: &str = "bold";
pub const ITALIC_ATTR: &str = "italic";
pub const HREF_ATTR: &str = "href";
pub const CODE_ATTR: &str = "code";
pub const FORMULA_ATTR: &str = "formula";
pub const STRIKETHROUGH_ATTR: &str = "strikethrough";
pub const INLINE_MATH_SYMBOL: &str = "$";

// Table Keys
pub const ROWS_LEN_FIELD: &str = "rowsLen";
pub const COLS_LEN_FIELD: &str = "colsLen";
pub const COL_DEFAULT_WIDTH_FIELD: &str = "colDefaultWidth";
pub const ROW_DEFAULT_HEIGHT_FIELD: &str = "rowDefaultHeight";
pub const ROW_POSITION_FIELD: &str = "rowPosition";
pub const COL_POSITION_FIELD: &str = "colPosition";

// List Keys
pub const CHECKED_FIELD: &str = "checked";
pub const START_NUMBER_FIELD: &str = "number";

pub const ALIGN_FIELD: &str = "align";
