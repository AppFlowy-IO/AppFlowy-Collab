use crate::blocks::{Block, TextDelta};
use std::collections::HashMap;

#[inline]
pub(crate) fn push_deltas_to_str(buf: &mut String, deltas: Vec<TextDelta>) {
  for delta in deltas {
    if let TextDelta::Inserted(text, _) = delta {
      let trimmed = text.trim();
      if !trimmed.is_empty() {
        buf.push_str(trimmed);
      }
    }
  }
}

/// Try to retrieve deltas from `block.data.delta`.
#[inline]
pub(crate) fn get_delta_from_block_data(block: &Block) -> Option<Vec<TextDelta>> {
  if let Some(delta) = block.data.get("delta") {
    if let Ok(deltas) = serde_json::from_value::<Vec<TextDelta>>(delta.clone()) {
      return Some(deltas);
    }
  }
  None
}

/// Try to retrieve deltas from text_map's text associated with `block.external_id`.
#[inline]
pub(crate) fn get_delta_from_external_text_id(
  block: &Block,
  text_map: &mut HashMap<String, Vec<TextDelta>>,
) -> Option<Vec<TextDelta>> {
  if block.external_type.as_deref() == Some("text") {
    if let Some(text_id) = block.external_id.as_deref() {
      if let Some(deltas) = text_map.remove(text_id) {
        return Some(deltas);
      }
    }
  }
  None
}
