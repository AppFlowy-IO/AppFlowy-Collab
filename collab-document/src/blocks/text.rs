use crate::blocks::text_entities::TextDelta;
use collab::preclude::*;
use collab::util::TextExt;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

pub struct TextOperation {
  root: MapRef,
}

impl TextOperation {
  pub fn new(root: MapRef) -> Self {
    Self { root }
  }

  /// get text ref wrapper with text_id
  pub fn get_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str) -> TextRef {
    self.root.get_or_init_text(txn, text_id)
  }

  /// delete text ref wrapper with text_id
  pub fn delete_text_with_txn(&self, txn: &mut TransactionMut, text_id: &str) {
    self.root.remove(txn, text_id);
  }

  /// get text delta with text_id
  pub fn get_delta_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Option<Vec<TextDelta>> {
    let value = self.root.get(txn, text_id)?;
    let text_ref: TextRef = value.cast().ok()?;
    Some(
      text_ref
        .delta(txn)
        .iter()
        .map(|d| TextDelta::from(d.clone().map(|s| s.to_string(txn))))
        .collect(),
    )
  }

  /// Applies provided delta to the text with the given `text_id`. If no text with such ID existed,
  /// it will always be created by the end of this mehtod call.
  pub fn apply_delta(&self, txn: &mut TransactionMut, text_id: &str, delta: Vec<TextDelta>) {
    let text_ref = self.get_text_with_txn(txn, text_id);
    if !delta.is_empty() {
      let delta: Vec<Delta<In>> = delta.into_iter().map(|d| d.to_delta()).collect();
      text_ref.apply_delta(txn, delta);
    }
  }

  pub fn set_delta(&self, txn: &mut TransactionMut, text_id: &str, delta: Vec<TextDelta>) {
    let text_ref = self.get_text_with_txn(txn, text_id);

    // remove all deltas
    let len = text_ref.len(txn);
    text_ref.remove_range(txn, 0, len);

    // apply new deltas
    let delta: Vec<Delta<In>> = delta.into_iter().map(|d| d.to_delta()).collect();
    text_ref.apply_delta(txn, delta);
  }

  /// get all text delta and serialize to json string
  pub fn serialize_all_text_delta<T: ReadTxn>(&self, txn: &T) -> HashMap<String, String> {
    self
      .root
      .iter(txn)
      .filter_map(|(k, _)| {
        self.get_delta_with_txn(txn, k).map(|delta| {
          (
            k.to_string(),
            serde_json::to_string(&delta).unwrap_or_default(),
          )
        })
      })
      .collect()
  }

  pub fn all_text_delta<T: ReadTxn>(&self, txn: &T) -> HashMap<String, Vec<TextDelta>> {
    self
      .root
      .iter(txn)
      .filter_map(|(k, _)| {
        self
          .get_delta_with_txn(txn, k)
          .map(|delta| (k.to_string(), delta))
      })
      .collect()
  }

  /// get all text delta and join as string
  pub fn stringify_all_text_delta<T: ReadTxn>(&self, txn: &T) -> HashMap<String, String> {
    self
      .root
      .iter(txn)
      .filter_map(|(k, _)| {
        self.get_delta_with_txn(txn, k).map(|delta| {
          let text: Vec<String> = delta
            .iter()
            .filter_map(|d| match d {
              TextDelta::Inserted(s, _) => Some(s.clone()),
              _ => None,
            })
            .collect();
          let text = text.join("");
          (k.to_string(), text)
        })
      })
      .collect()
  }
}

pub fn mention_block_data(view_id: &str, parent_view_id: &str) -> HashMap<String, JsonValue> {
  let mut data = HashMap::with_capacity(2);
  data.insert("view_id".to_string(), json!(view_id));
  data.insert("parent_id".to_string(), json!(parent_view_id));
  data
}

pub fn extract_view_id_from_block_data(data: &HashMap<String, JsonValue>) -> Option<String> {
  data
    .get("view_id")
    .and_then(|v| v.as_str().map(|s| s.to_string()))
}

pub fn mention_block_delta(view_id: &str) -> TextDelta {
  let mut mention_content = HashMap::with_capacity(2);
  mention_content.insert("type".to_string(), "page".to_string());
  mention_content.insert("page_id".to_string(), view_id.to_string());

  let mut mention = Attrs::with_capacity(1);
  mention.insert("mention".into(), Any::from(mention_content));
  TextDelta::Inserted("$".to_string(), Some(mention))
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Hash)]
pub struct MentionBlockContent {
  pub ty: String,
  pub page_id: String,
}

pub fn mention_block_content_from_delta(delta: &TextDelta) -> Option<MentionBlockContent> {
  match delta {
    TextDelta::Inserted(_, Some(attrs)) => {
      if let Some(Any::Map(attrs)) = attrs.get("mention") {
        let ty = attrs.get("type")?.to_string();
        let page_id = attrs.get("page_id")?.to_string();
        Some(MentionBlockContent { ty, page_id })
      } else {
        None
      }
    },
    _ => None,
  }
}

pub fn extract_page_id_from_block_delta(deltas: &[TextDelta]) -> Option<String> {
  deltas
    .iter()
    .filter_map(|d| match d {
      TextDelta::Inserted(_, Some(attrs)) => {
        if let Some(Any::Map(attrs)) = attrs.get("mention") {
          attrs.get("page_id").map(|v| v.to_string())
        } else {
          None
        }
      },
      _ => None,
    })
    .next()
}
