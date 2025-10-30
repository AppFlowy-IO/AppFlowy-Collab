use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::HashMap;
use std::fmt::Debug;

use super::traits::{DocumentParserDelegate, ParseContext};
use crate::document::blocks::{Block, BlockType};

use crate::preclude::{Any, Attrs};

const MENTION_KEY: &str = "mention";
const MENTION_TYPE_KEY: &str = "type";
const PERSON_TYPE: &str = "person";
const PAGE_TYPE: &str = "page";
const CHILD_PAGE_TYPE: &str = "childPage";
const DATE_TYPE: &str = "date";
const REMINDER_TYPE: &str = "reminder";
const EXTERNAL_LINK_TYPE: &str = "externalLink";

/// Trait used to customise how plain text export handles mention attributes and embed blocks.
///
/// The trait extends [`DocumentParserDelegate`] so it can hook into the existing delta visitor
/// infrastructure, while also offering a block-level resolution hook via
/// [`PlainTextResolver::resolve_block_text`].
pub trait PlainTextResolver: DocumentParserDelegate + Debug + Send + Sync {
  /// Resolve a block (such as sub page, file, image) into a plain-text representation.
  ///
  /// Returning `None` lets the default parser formatting kick in.
  fn resolve_block_text(&self, _block: &Block, _context: &ParseContext) -> Option<String> {
    None
  }
}

/// Default implementation that relies on data already present in the document delta.
///
/// Consumers can supply override maps to control rendered labels for person and document ids.
#[derive(Debug, Default, Clone)]
pub struct DefaultPlainTextResolver {
  person_names: HashMap<String, String>,
  document_titles: HashMap<String, String>,
}

impl DefaultPlainTextResolver {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with_person_names(mut self, map: HashMap<String, String>) -> Self {
    self.person_names = map;
    self
  }

  pub fn with_document_titles(mut self, map: HashMap<String, String>) -> Self {
    self.document_titles = map;
    self
  }

  pub fn set_person_names(&mut self, map: HashMap<String, String>) {
    self.person_names = map;
  }

  pub fn set_document_titles(&mut self, map: HashMap<String, String>) {
    self.document_titles = map;
  }

  fn title_for_document(&self, page_id: &str) -> String {
    self
      .document_titles
      .get(page_id)
      .cloned()
      .unwrap_or_else(|| page_id.to_string())
  }

  fn label_for_person(&self, person_id: Option<&str>, person_name: Option<&str>) -> Option<String> {
    let id = person_id?;
    let base_name = self
      .person_names
      .get(id)
      .cloned()
      .or_else(|| person_name.map(ToString::to_string))
      .unwrap_or_else(|| id.to_string());
    Some(format!("@{}", base_name))
  }

  fn label_for_document(&self, page_id: Option<&str>) -> Option<String> {
    let page_id = page_id?;
    Some(format!("[[{}]]", self.title_for_document(page_id)))
  }

  fn label_for_date(&self, date: Option<&str>, include_time: bool) -> Option<String> {
    let date_value = date?;
    let formatted =
      parse_date_string(date_value, include_time).unwrap_or_else(|| date_value.to_string());
    Some(format!("@{}", formatted))
  }

  fn label_for_external_link(&self, url: Option<&str>) -> Option<String> {
    let url = url?;
    if url.is_empty() {
      None
    } else {
      Some(url.to_string())
    }
  }
}

impl DocumentParserDelegate for DefaultPlainTextResolver {
  fn handle_text_delta(
    &self,
    text: &str,
    attributes: Option<&Attrs>,
    _context: &ParseContext,
  ) -> Option<String> {
    if text != "$" {
      return None;
    }

    let mention = MentionInfo::from_attrs(attributes)?;
    match mention.kind.as_str() {
      PERSON_TYPE => self.label_for_person(
        mention.string("person_id").as_deref(),
        mention.string("person_name").as_deref(),
      ),
      PAGE_TYPE | CHILD_PAGE_TYPE => self.label_for_document(mention.string("page_id").as_deref()),
      DATE_TYPE | REMINDER_TYPE => self.label_for_date(
        mention.string("date").as_deref(),
        mention.bool("include_time").unwrap_or(false),
      ),
      EXTERNAL_LINK_TYPE => self.label_for_external_link(mention.string("url").as_deref()),
      _ => None,
    }
  }
}

impl PlainTextResolver for DefaultPlainTextResolver {
  fn resolve_block_text(&self, block: &Block, _context: &ParseContext) -> Option<String> {
    match BlockType::from_block_ty(block.ty.as_str()) {
      BlockType::SubPage => {
        let view_id = block
          .data
          .get("view_id")
          .or_else(|| block.data.get("viewId"));
        view_id
          .and_then(|value| value.as_str())
          .map(|id| self.title_for_document(id))
      },
      BlockType::LinkPreview => block
        .data
        .get("url")
        .and_then(|value| value.as_str())
        .filter(|url| !url.is_empty())
        .map(|url| url.to_string()),
      BlockType::File => {
        let name = block.data.get("name").and_then(|value| value.as_str());
        let url = block.data.get("url").and_then(|value| value.as_str());
        name
          .map(|n| {
            if let Some(u) = url {
              if !u.is_empty() {
                format!("{} ({})", n, u)
              } else {
                n.to_string()
              }
            } else {
              n.to_string()
            }
          })
          .or_else(|| url.map(|u| u.to_string()))
      },
      BlockType::Image => block
        .data
        .get("url")
        .and_then(|value| value.as_str())
        .filter(|url| !url.is_empty())
        .map(|url| url.to_string()),
      _ => None,
    }
  }
}

#[derive(Debug, Clone)]
struct MentionInfo {
  kind: String,
  data: JsonMap<String, JsonValue>,
}

impl MentionInfo {
  fn from_attrs(attrs: Option<&Attrs>) -> Option<Self> {
    let attrs = attrs?;
    let mention_any = attrs.get(MENTION_KEY)?;

    let mention_map = mention_any_to_map(mention_any)?;
    let kind = mention_map
      .get(MENTION_TYPE_KEY)
      .and_then(|value| value.as_str())
      .unwrap_or_default()
      .to_string();

    Some(Self {
      kind,
      data: mention_map,
    })
  }

  fn string(&self, key: &str) -> Option<String> {
    self
      .data
      .get(key)
      .and_then(|value| value.as_str())
      .map(|value| value.to_string())
  }

  fn bool(&self, key: &str) -> Option<bool> {
    self.data.get(key).and_then(|value| match value {
      JsonValue::Bool(flag) => Some(*flag),
      JsonValue::String(text) => text.parse::<bool>().ok(),
      _ => None,
    })
  }
}

fn mention_any_to_map(mention_any: &Any) -> Option<JsonMap<String, JsonValue>> {
  let serialized = serde_json::to_string(mention_any).ok()?;

  if let Ok(map) = serde_json::from_str::<JsonMap<String, JsonValue>>(&serialized) {
    return Some(map);
  }

  let inner = serde_json::from_str::<String>(&serialized).ok()?;
  serde_json::from_str::<JsonMap<String, JsonValue>>(&inner).ok()
}

fn parse_date_string(input: &str, include_time: bool) -> Option<String> {
  if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
    return Some(if include_time {
      datetime
        .with_timezone(&Utc)
        .format("%Y-%m-%d %H:%M")
        .to_string()
    } else {
      datetime.date_naive().format("%Y-%m-%d").to_string()
    });
  }

  if include_time {
    if let Ok(datetime) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
      return Some(datetime.format("%Y-%m-%d %H:%M").to_string());
    }
  } else if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
    return Some(date.format("%Y-%m-%d").to_string());
  }

  None
}
