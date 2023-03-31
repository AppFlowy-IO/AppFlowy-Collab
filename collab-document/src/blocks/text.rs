use collab::{core::text_wrapper::TextDelta, preclude::*};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use serde_json::Value::Null;

pub struct TextMap {
  pub root: MapRefWrapper,
}

struct InsertedDelta {
  insert: String,
  attributes: Option<Box<Attrs>>,
}

impl InsertedDelta {
  fn new(insert: String, attributes: Option<Box<Attrs>>) -> Self {
    Self { insert, attributes }
  }
}

impl Serialize for InsertedDelta {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry("insert", &self.insert)?;
    let attrs = match &self.attributes {
      Some(attrs) => {
        let mut attrs_obj = serde_json::json!({});
        attrs.iter().for_each(|(k, v)| {
          attrs_obj[k.to_string()] = v.to_string().parse().unwrap_or_default();
        });
        attrs_obj
      },
      None => Null,
    };
    map.serialize_entry("attributes", &attrs)?;
    map.end()
  }
}

impl Serialize for TextMap {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let txn = self.root.transact();
    let mut map = serializer.serialize_map(Some(self.root.len(&txn) as usize))?;
    for (key, _) in self.root.iter(&txn) {
      let text = self.get_delta_with_txn(&txn, key);
      let value = serde_json::json!(text
        .iter()
        .map(|delta| match delta {
          Delta::Inserted(content, attrs) => {
            serde_json::json!(InsertedDelta::new(content.to_string(), attrs.clone()))
          },
          _ => Null,
        })
        .collect::<Vec<serde_json::Value>>());
      map.serialize_entry(key, &value)?;
    }
    map.end()
  }
}

impl TextMap {
  pub fn new(root: MapRefWrapper) -> Self {
    Self { root }
  }

  pub fn to_json(&self) -> serde_json::Value {
    serde_json::to_value(self).unwrap_or_default()
  }

  pub fn create_text(&self, txn: &mut TransactionMut, text_id: &str) -> TextRefWrapper {
    self.root.insert_text_with_txn(txn, text_id)
  }

  pub fn get_text(&self, text_id: &str) -> Option<TextRefWrapper> {
    let txn = self.root.transact();
    self.root.get_text_ref_with_txn(&txn, text_id)
  }

  pub fn apply_text_delta_with_txn(
    &self,
    txn: &mut TransactionMut,
    text_id: &str,
    delta: Vec<TextDelta>,
  ) {
    let text_ref = self
      .get_text(text_id)
      .unwrap_or_else(|| self.create_text(txn, text_id));
    text_ref.apply_delta_with_txn(txn, delta);
  }

  pub fn get_str(&self, text_id: &str) -> Option<String> {
    let txn = self.root.transact();
    self.get_str_with_txn(&txn, text_id)
  }

  pub fn get_str_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Option<String> {
    self
      .root
      .get_text_ref_with_txn(txn, text_id)
      .map(|map| map.get_string(txn))
  }

  pub fn get_delta(&self, text_id: &str) -> Vec<YrsDelta> {
    let txn = self.root.transact();
    self
      .root
      .get_text_ref_with_txn(&txn, text_id)
      .map(|map| map.get_delta_with_txn(&txn))
      .unwrap_or_default()
  }

  pub fn get_delta_with_txn<T: ReadTxn>(&self, txn: &T, text_id: &str) -> Vec<YrsDelta> {
    self
      .root
      .get_text_ref_with_txn(txn, text_id)
      .map(|map| map.get_delta_with_txn(txn))
      .unwrap_or_default()
  }

  pub fn delete_with_txn(&self, txn: &mut TransactionMut, text_id: &str) {
    self.root.delete_with_txn(txn, text_id);
  }
}
