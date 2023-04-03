use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub enum BlockType {
  Page,
  Text,
  Heading,
  // Image,
  // Custom,
}

impl ToString for BlockType {
  fn to_string(&self) -> String {
    match self {
      BlockType::Page => "Page",
      BlockType::Text => "Text",
      BlockType::Heading => "Heading",
      // BlockType::Image => "Image",
      // BlockType::Custom => "Custom",
    }
    .to_string()
  }
}

impl BlockType {
  pub fn from_string(s: &str) -> Self {
    match s {
      "Page" => BlockType::Page,
      "Text" => BlockType::Text,
      "Heading" => BlockType::Heading,
      // "Image" => BlockType::Image,
      // "Custom" => BlockType::Custom,
      _ => BlockType::Text,
    }
  }
}
#[derive(Serialize, Deserialize, Debug)]
pub enum BlockDataEnum {
  Page(String),
  Text(String),
  Heading(u32, String),
  // Image(),
  // Custom(HashMap<String, Value>),
}

impl ToString for BlockDataEnum {
  fn to_string(&self) -> String {
    serde_json::to_string(self).unwrap_or_else(|_| "".to_string())
  }
}

impl BlockDataEnum {
  pub fn from_string(s: &str) -> Self {
    serde_json::from_str(s).unwrap_or_else(|_| BlockDataEnum::Text("".to_string()))
  }

  pub fn from_map(ty: BlockType, map: &HashMap<String, Value>) -> Self {
    let text = map
      .get("text")
      .unwrap_or(&Value::String("".to_string()))
      .to_string();

    match ty {
      BlockType::Page => BlockDataEnum::Page(text),
      BlockType::Text => BlockDataEnum::Text(text),
      BlockType::Heading => BlockDataEnum::Heading(
        map
          .get("level")
          .unwrap_or(&Value::Number(0.into()))
          .to_string()
          .parse()
          .unwrap_or_default(),
        text,
      ),
      // BlockType::Image => BlockDataEnum::Image(),
      // BlockType::Custom => BlockDataEnum::Custom(map.clone()),
    }
  }

  pub fn get_text(&self) -> Option<String> {
    match self {
      BlockDataEnum::Page(text) | BlockDataEnum::Text(text) => Some(text.clone()),
      BlockDataEnum::Heading(_, text) => Some(text.clone()),
      // _ => None,
    }
  }

  pub fn to_json_value(&self) -> Value {
    match self {
      BlockDataEnum::Page(text) | BlockDataEnum::Text(text) => serde_json::json!({
        "text": text,
      }),
      BlockDataEnum::Heading(level, text) => serde_json::json!({
        "level": level,
        "text": text,
      }),
      // BlockDataEnum::Custom(map) => {
      //   serde_json::to_value(map).unwrap_or_else(|_| serde_json::json!({}))
      // },
      // _ => serde_json::json!({}),
    }
  }
}
