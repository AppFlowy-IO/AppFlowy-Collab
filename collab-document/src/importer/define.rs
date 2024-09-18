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
}

impl BlockType {
  pub fn as_str(&self) -> &'static str {
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
    }
  }

  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "page" => Some(BlockType::Page),
      "paragraph" => Some(BlockType::Paragraph),
      "heading" => Some(BlockType::Heading),
      "quote" => Some(BlockType::Quote),
      "todo_list" => Some(BlockType::TodoList),
      "numbered_list" => Some(BlockType::NumberedList),
      "bulleted_list" => Some(BlockType::BulletedList),
      "image" => Some(BlockType::Image),
      "link_preview" => Some(BlockType::LinkPreview),
      "code" => Some(BlockType::Code),
      "math_equation" => Some(BlockType::MathEquation),
      "divider" => Some(BlockType::Divider),
      "table" => Some(BlockType::Table),
      "table/cell" => Some(BlockType::TableCell),
      "text" => Some(BlockType::Text),
      _ => None,
    }
  }
}

// Implement AsRef<str> for ContentType
impl AsRef<str> for BlockType {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

impl ToString for BlockType {
  fn to_string(&self) -> String {
    self.as_str().to_string()
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
