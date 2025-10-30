/// Mention block helper functions that mirror Flutter's MentionBlockKeys utilities
///
/// This module provides helper functions to create mention block deltas following
/// the same structure as Flutter's `buildMentionXXXAttributes` functions.
///
/// All mention blocks use the special character '$' as the inserted text, with
/// attributes containing the mention metadata.
use super::text_entities::TextDelta;
use crate::preclude::{Any, Attrs};
use std::collections::HashMap;

/// The special character used for all mention blocks
pub const MENTION_CHAR: &str = "$";

/// Attribute keys used in mention blocks
pub mod mention_keys {
  pub const MENTION: &str = "mention";
  pub const TYPE: &str = "type";
  pub const PAGE_ID: &str = "page_id";
  pub const BLOCK_ID: &str = "block_id";
  pub const ROW_ID: &str = "row_id";
  pub const URL: &str = "url";
  pub const DATE: &str = "date";
  pub const INCLUDE_TIME: &str = "include_time";
  pub const REMINDER_ID: &str = "reminder_id";
  pub const REMINDER_OPTION: &str = "reminder_option";
  pub const PERSON_ID: &str = "person_id";
  pub const PERSON_NAME: &str = "person_name";
}

/// Mention type constants
pub mod mention_types {
  pub const PERSON: &str = "person";
  pub const PAGE: &str = "page";
  pub const CHILD_PAGE: &str = "childPage";
  pub const DATE: &str = "date";
  pub const REMINDER: &str = "reminder"; // Backward compatibility alias for 'date'
  pub const EXTERNAL_LINK: &str = "externalLink";
}

/// Builder for person mention attributes
///
/// # Example
/// ```
/// use collab::document::blocks::*;
///
/// let delta = build_mention_person_delta(
///     "user123",
///     "John Doe",
///     "doc456",
///     Some("block789"),
///     None,
/// );
/// ```
pub fn build_mention_person_delta(
  person_id: &str,
  person_name: &str,
  page_id: &str,
  block_id: Option<&str>,
  row_id: Option<&str>,
) -> TextDelta {
  let mut mention_content = HashMap::new();
  mention_content.insert(
    mention_keys::TYPE.to_string(),
    mention_types::PERSON.to_string(),
  );
  mention_content.insert(mention_keys::PERSON_ID.to_string(), person_id.to_string());
  mention_content.insert(
    mention_keys::PERSON_NAME.to_string(),
    person_name.to_string(),
  );
  mention_content.insert(mention_keys::PAGE_ID.to_string(), page_id.to_string());

  if let Some(block_id) = block_id {
    mention_content.insert(mention_keys::BLOCK_ID.to_string(), block_id.to_string());
  }
  if let Some(row_id) = row_id {
    mention_content.insert(mention_keys::ROW_ID.to_string(), row_id.to_string());
  }

  let mut attrs = Attrs::new();
  attrs.insert(mention_keys::MENTION.into(), Any::from(mention_content));

  TextDelta::Inserted(MENTION_CHAR.to_string(), Some(attrs))
}

/// Builder for page/childPage mention attributes
///
/// # Example
/// ```
/// use collab::document::blocks::*;
///
/// // Regular page mention
/// let page_delta = build_mention_page_delta(
///     MentionPageType::Page,
///     "page123",
///     Some("block456"),
///     None,
/// );
///
/// // Child page mention
/// let child_delta = build_mention_page_delta(
///     MentionPageType::ChildPage,
///     "page789",
///     None,
///     None,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionPageType {
  Page,
  ChildPage,
}

impl MentionPageType {
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Page => mention_types::PAGE,
      Self::ChildPage => mention_types::CHILD_PAGE,
    }
  }
}

pub fn build_mention_page_delta(
  mention_type: MentionPageType,
  page_id: &str,
  block_id: Option<&str>,
  row_id: Option<&str>,
) -> TextDelta {
  let mut mention_content = HashMap::new();
  mention_content.insert(
    mention_keys::TYPE.to_string(),
    mention_type.as_str().to_string(),
  );
  mention_content.insert(mention_keys::PAGE_ID.to_string(), page_id.to_string());

  if let Some(block_id) = block_id {
    mention_content.insert(mention_keys::BLOCK_ID.to_string(), block_id.to_string());
  }
  if let Some(row_id) = row_id {
    mention_content.insert(mention_keys::ROW_ID.to_string(), row_id.to_string());
  }

  let mut attrs = Attrs::new();
  attrs.insert(mention_keys::MENTION.into(), Any::from(mention_content));

  TextDelta::Inserted(MENTION_CHAR.to_string(), Some(attrs))
}

/// Builder for date/reminder mention attributes
///
/// # Example
/// ```
/// use collab::document::blocks::*;
///
/// let delta = build_mention_date_delta(
///     "2025-01-30T10:00:00Z",
///     Some("reminder123"),
///     Some("atTimeOfEvent"),
///     true, // include_time
/// );
/// ```
pub fn build_mention_date_delta(
  date: &str,
  reminder_id: Option<&str>,
  reminder_option: Option<&str>,
  include_time: bool,
) -> TextDelta {
  let mut mention_content = HashMap::new();
  mention_content.insert(
    mention_keys::TYPE.to_string(),
    mention_types::DATE.to_string(),
  );
  mention_content.insert(mention_keys::DATE.to_string(), date.to_string());
  mention_content.insert(
    mention_keys::INCLUDE_TIME.to_string(),
    include_time.to_string(),
  );

  if let Some(reminder_id) = reminder_id {
    mention_content.insert(
      mention_keys::REMINDER_ID.to_string(),
      reminder_id.to_string(),
    );
  }
  if let Some(reminder_option) = reminder_option {
    mention_content.insert(
      mention_keys::REMINDER_OPTION.to_string(),
      reminder_option.to_string(),
    );
  }

  let mut attrs = Attrs::new();
  attrs.insert(mention_keys::MENTION.into(), Any::from(mention_content));

  TextDelta::Inserted(MENTION_CHAR.to_string(), Some(attrs))
}

/// Builder for external link mention attributes
///
/// # Example
/// ```
/// use collab::document::blocks::*;
///
/// let delta = build_mention_external_link_delta("https://example.com");
/// ```
pub fn build_mention_external_link_delta(url: &str) -> TextDelta {
  let mut mention_content = HashMap::new();
  mention_content.insert(
    mention_keys::TYPE.to_string(),
    mention_types::EXTERNAL_LINK.to_string(),
  );
  mention_content.insert(mention_keys::URL.to_string(), url.to_string());

  let mut attrs = Attrs::new();
  attrs.insert(mention_keys::MENTION.into(), Any::from(mention_content));

  TextDelta::Inserted(MENTION_CHAR.to_string(), Some(attrs))
}

/// Extract mention type from a TextDelta
///
/// Returns None if the delta is not a mention or doesn't have a type field
pub fn extract_mention_type(delta: &TextDelta) -> Option<String> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        mention_map.get(mention_keys::TYPE).map(|v| v.to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Extract person ID from a person mention delta
pub fn extract_person_id(delta: &TextDelta) -> Option<String> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        mention_map
          .get(mention_keys::PERSON_ID)
          .map(|v| v.to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Extract page ID from a page/childPage mention delta
pub fn extract_page_id(delta: &TextDelta) -> Option<String> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        mention_map
          .get(mention_keys::PAGE_ID)
          .map(|v| v.to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Extract date from a date/reminder mention delta
pub fn extract_date(delta: &TextDelta) -> Option<String> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        mention_map.get(mention_keys::DATE).map(|v| v.to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Extract URL from an external link mention delta
pub fn extract_url(delta: &TextDelta) -> Option<String> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        mention_map.get(mention_keys::URL).map(|v| v.to_string())
      } else {
        None
      }
    },
    _ => None,
  }
}

/// Check if a delta is a mention block
pub fn is_mention(delta: &TextDelta) -> bool {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      attrs.contains_key(mention_keys::MENTION)
    },
    _ => false,
  }
}

/// Comprehensive mention data extractor
#[derive(Debug, Clone, PartialEq)]
pub enum MentionData {
  Person {
    person_id: String,
    person_name: String,
    page_id: String,
    block_id: Option<String>,
    row_id: Option<String>,
  },
  Page {
    page_id: String,
    block_id: Option<String>,
    row_id: Option<String>,
  },
  ChildPage {
    page_id: String,
  },
  Date {
    date: String,
    include_time: bool,
    reminder_id: Option<String>,
    reminder_option: Option<String>,
  },
  ExternalLink {
    url: String,
  },
}

/// Extract all mention data from a TextDelta
///
/// Returns Some(MentionData) if the delta is a valid mention, None otherwise
pub fn extract_mention_data(delta: &TextDelta) -> Option<MentionData> {
  match delta {
    TextDelta::Inserted(text, Some(attrs)) if text == MENTION_CHAR => {
      if let Some(Any::Map(mention_map)) = attrs.get(mention_keys::MENTION) {
        let mention_type = mention_map.get(mention_keys::TYPE)?.to_string();

        match mention_type.as_str() {
          mention_types::PERSON => Some(MentionData::Person {
            person_id: mention_map.get(mention_keys::PERSON_ID)?.to_string(),
            person_name: mention_map.get(mention_keys::PERSON_NAME)?.to_string(),
            page_id: mention_map.get(mention_keys::PAGE_ID)?.to_string(),
            block_id: mention_map
              .get(mention_keys::BLOCK_ID)
              .map(|v| v.to_string()),
            row_id: mention_map.get(mention_keys::ROW_ID).map(|v| v.to_string()),
          }),
          mention_types::PAGE => Some(MentionData::Page {
            page_id: mention_map.get(mention_keys::PAGE_ID)?.to_string(),
            block_id: mention_map
              .get(mention_keys::BLOCK_ID)
              .map(|v| v.to_string()),
            row_id: mention_map.get(mention_keys::ROW_ID).map(|v| v.to_string()),
          }),
          mention_types::CHILD_PAGE => Some(MentionData::ChildPage {
            page_id: mention_map.get(mention_keys::PAGE_ID)?.to_string(),
          }),
          mention_types::DATE | mention_types::REMINDER => {
            let date = mention_map.get(mention_keys::DATE)?.to_string();
            let include_time = mention_map
              .get(mention_keys::INCLUDE_TIME)
              .and_then(|v| v.to_string().parse::<bool>().ok())
              .unwrap_or(false);
            Some(MentionData::Date {
              date,
              include_time,
              reminder_id: mention_map
                .get(mention_keys::REMINDER_ID)
                .map(|v| v.to_string()),
              reminder_option: mention_map
                .get(mention_keys::REMINDER_OPTION)
                .map(|v| v.to_string()),
            })
          },
          mention_types::EXTERNAL_LINK => Some(MentionData::ExternalLink {
            url: mention_map.get(mention_keys::URL)?.to_string(),
          }),
          _ => None,
        }
      } else {
        None
      }
    },
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_person_mention() {
    let delta = build_mention_person_delta("user123", "John Doe", "doc456", Some("block789"), None);

    assert!(is_mention(&delta));
    assert_eq!(extract_mention_type(&delta), Some("person".to_string()));
    assert_eq!(extract_person_id(&delta), Some("user123".to_string()));

    if let Some(MentionData::Person {
      person_id,
      person_name,
      page_id,
      block_id,
      row_id,
    }) = extract_mention_data(&delta)
    {
      assert_eq!(person_id, "user123");
      assert_eq!(person_name, "John Doe");
      assert_eq!(page_id, "doc456");
      assert_eq!(block_id, Some("block789".to_string()));
      assert_eq!(row_id, None);
    } else {
      panic!("Expected Person mention data");
    }
  }

  #[test]
  fn test_page_mention() {
    let delta = build_mention_page_delta(MentionPageType::Page, "page123", Some("block456"), None);

    assert!(is_mention(&delta));
    assert_eq!(extract_mention_type(&delta), Some("page".to_string()));
    assert_eq!(extract_page_id(&delta), Some("page123".to_string()));

    if let Some(MentionData::Page {
      page_id,
      block_id,
      row_id,
    }) = extract_mention_data(&delta)
    {
      assert_eq!(page_id, "page123");
      assert_eq!(block_id, Some("block456".to_string()));
      assert_eq!(row_id, None);
    } else {
      panic!("Expected Page mention data");
    }
  }

  #[test]
  fn test_child_page_mention() {
    let delta = build_mention_page_delta(MentionPageType::ChildPage, "page789", None, None);

    assert!(is_mention(&delta));
    assert_eq!(extract_mention_type(&delta), Some("childPage".to_string()));
    assert_eq!(extract_page_id(&delta), Some("page789".to_string()));

    if let Some(MentionData::ChildPage { page_id }) = extract_mention_data(&delta) {
      assert_eq!(page_id, "page789");
    } else {
      panic!("Expected ChildPage mention data");
    }
  }

  #[test]
  fn test_date_mention() {
    let delta = build_mention_date_delta(
      "2025-01-30T12:00:00.000Z",
      Some("reminder123"),
      Some("atTimeOfEvent"),
      true,
    );

    assert!(is_mention(&delta));
    assert_eq!(extract_mention_type(&delta), Some("date".to_string()));
    assert_eq!(
      extract_date(&delta),
      Some("2025-01-30T12:00:00.000Z".to_string())
    );

    if let Some(MentionData::Date {
      date,
      include_time,
      reminder_id,
      reminder_option,
    }) = extract_mention_data(&delta)
    {
      assert_eq!(date, "2025-01-30T12:00:00.000Z");
      assert!(include_time);
      assert_eq!(reminder_id, Some("reminder123".to_string()));
      assert_eq!(reminder_option, Some("atTimeOfEvent".to_string()));
    } else {
      panic!("Expected Date mention data");
    }
  }

  #[test]
  fn test_external_link_mention() {
    let delta = build_mention_external_link_delta("https://example.com");

    assert!(is_mention(&delta));
    assert_eq!(
      extract_mention_type(&delta),
      Some("externalLink".to_string())
    );
    assert_eq!(extract_url(&delta), Some("https://example.com".to_string()));

    if let Some(MentionData::ExternalLink { url }) = extract_mention_data(&delta) {
      assert_eq!(url, "https://example.com");
    } else {
      panic!("Expected ExternalLink mention data");
    }
  }

  #[test]
  fn test_non_mention_delta() {
    let delta = TextDelta::Inserted("Regular text".to_string(), None);

    assert!(!is_mention(&delta));
    assert_eq!(extract_mention_type(&delta), None);
    assert_eq!(extract_mention_data(&delta), None);
  }
}
