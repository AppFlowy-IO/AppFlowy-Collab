use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use collab::preclude::{Any, Attrs, Delta, YrsInput};

const FIELD_INSERT: &str = "insert";
const FIELD_DELETE: &str = "delete";
const FIELD_RETAIN: &str = "retain";
const FIELD_ATTRIBUTES: &str = "attributes";
const FIELDS: &[&str] = &[FIELD_INSERT, FIELD_DELETE, FIELD_RETAIN, FIELD_ATTRIBUTES];

#[derive(Debug, Clone)]
pub enum TextDelta {
  /// Determines a change that resulted in insertion of a piece of text, which optionally could have been
  /// formatted with provided set of attributes.
  Inserted(String, Option<Attrs>),

  /// Determines a change that resulted in removing a consecutive range of characters.
  Deleted(u32),

  /// Determines a number of consecutive unchanged characters. Used to recognize non-edited spaces
  /// between [Delta::Inserted] and/or [Delta::Deleted] chunks. Can contain an optional set of
  /// attributes, which have been used to format an existing piece of text.
  Retain(u32, Option<Attrs>),
}

impl PartialEq for TextDelta {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Inserted(content1, attrs1), Self::Inserted(content2, attrs2)) => {
        content1 == content2 && attrs1 == attrs2
      },
      (Self::Deleted(len1), Self::Deleted(len2)) => len1 == len2,
      (Self::Retain(len1, attrs1), Self::Retain(len2, attrs2)) => len1 == len2 && attrs1 == attrs2,
      _ => false,
    }
  }
}

impl Eq for TextDelta {}

impl TextDelta {
  pub fn from(value: Delta<String>) -> Self {
    match value {
      Delta::Inserted(content, attrs) => Self::Inserted(content, attrs.map(|attrs| *attrs)),
      Delta::Deleted(len) => Self::Deleted(len),
      Delta::Retain(len, attrs) => Self::Retain(len, attrs.map(|attrs| *attrs)),
    }
  }

  pub fn to_delta(self) -> Delta<YrsInput> {
    match self {
      Self::Inserted(content, attrs) => {
        let content = YrsInput::from(content);
        Delta::Inserted(content, attrs.map(Box::new))
      },
      Self::Deleted(len) => Delta::Deleted(len),
      Self::Retain(len, attrs) => Delta::Retain(len, attrs.map(Box::new)),
    }
  }
}

impl Serialize for TextDelta {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map = serializer.serialize_map(Some(2))?; // Start serializing a map with up to 2 entries.

    match self {
      Self::Inserted(content, attrs) => {
        // Serialize the "insert" field with its content.
        map.serialize_entry(FIELD_INSERT, content)?;
        serialize_optional_attributes(attrs, &mut map)?;
      },
      Self::Deleted(len) => {
        // Serialize the "delete" field with its length.
        map.serialize_entry(FIELD_DELETE, len)?;
      },
      Self::Retain(len, attrs) => {
        // Serialize the "retain" field with its length.
        map.serialize_entry(FIELD_RETAIN, len)?;
        serialize_optional_attributes(attrs, &mut map)?;
      },
    }

    // End the serialization of the map.
    map.end()
  }
}

fn serialize_optional_attributes<S>(attrs: &Option<Attrs>, map: &mut S) -> Result<(), S::Error>
where
  S: SerializeMap,
{
  if let Some(attrs) = attrs {
    // If there are attributes, serialize them as a HashMap.
    let attrs_hash = attrs
      .iter()
      .map(|(k, v)| (k.deref().to_string(), v.clone()))
      .collect::<HashMap<String, Any>>();
    // Serialize the "attributes" field.
    map.serialize_entry(FIELD_ATTRIBUTES, &attrs_hash)?;
  }
  Ok(())
}

impl<'de> Deserialize<'de> for TextDelta {
  fn deserialize<D>(deserializer: D) -> Result<TextDelta, D::Error>
  where
    D: Deserializer<'de>,
  {
    // Define a visitor for deserialization
    struct TextDeltaVisitor;

    impl<'de> Visitor<'de> for TextDeltaVisitor {
      type Value = TextDelta;

      // Describe what is expected for the deserialized value
      fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid TextDelta")
      }

      // Deserialize a map
      fn visit_map<A>(self, mut map: A) -> Result<TextDelta, A::Error>
      where
        A: MapAccess<'de>,
      {
        // Initialize variables to store deserialized fields
        let mut delta_type: Option<String> = None;
        let mut content: Option<String> = None;
        let mut len: Option<usize> = None;
        let mut attrs: Option<HashMap<Arc<str>, Any>> = None;

        // Deserialize each key-value pair in the map
        while let Some(key) = map.next_key::<String>()? {
          match key.as_str() {
            // Handle the "insert" field
            FIELD_INSERT => {
              // Check if the delta type is already set, and return an error if it's a duplicate field
              if delta_type.is_some() {
                return Err(de::Error::duplicate_field(FIELD_INSERT));
              }
              // Deserialize and store the "content" value
              content = Some(map.next_value()?);
              // Set the delta type to "insert"
              delta_type = Some(key);
            },
            // Handle the "delete" field
            FIELD_DELETE => {
              // Check if the delta type is already set, and return an error if it's a duplicate field
              if delta_type.is_some() {
                return Err(de::Error::duplicate_field(FIELD_DELETE));
              }
              // Deserialize and store the "len" value
              len = Some(map.next_value()?);
              // Set the delta type to "delete"
              delta_type = Some(key);
            },
            // Handle the "retain" field
            FIELD_RETAIN => {
              // Check if the delta type is already set, and return an error if it's a duplicate field
              if delta_type.is_some() {
                return Err(de::Error::duplicate_field(FIELD_RETAIN));
              }
              // Deserialize and store the "len" value
              len = Some(map.next_value()?);
              // Set the delta type to "retain"
              delta_type = Some(key);
            },
            // Handle the "attributes" field
            FIELD_ATTRIBUTES => {
              // If "attrs" is not initialized, create an empty HashMap to store attributes
              if attrs.is_none() {
                attrs = Some(HashMap::new());
              }
              // Deserialize the "attrs_val" value as a HashMap<String, Any>
              let attrs_val = map.next_value::<HashMap<String, Any>>()?;
              // Iterate through the attributes and insert them into the HashMap
              attrs.get_or_insert(HashMap::new()).extend(
                attrs_val
                  .iter()
                  .map(|(key, val)| (Arc::from(key.to_string()), val.clone())),
              );
            },
            // Handle unknown fields by returning an error
            _ => {
              return Err(de::Error::unknown_field(key.as_str(), FIELDS));
            },
          }
        }

        // Match the deserialized delta type and create a TextDelta variant
        match delta_type {
          Some(delta_type) => match delta_type.as_str() {
            // If "delta_type" is "insert," construct an "Inserted" TextDelta variant
            FIELD_INSERT => {
              // If "attrs" is Some, include attributes in the variant; otherwise, use None
              Ok(TextDelta::Inserted(
                content.ok_or_else(|| de::Error::missing_field(FIELD_INSERT))?,
                attrs,
              ))
            },
            // If "delta_type" is "delete," construct a "Deleted" TextDelta variant
            FIELD_DELETE => Ok(TextDelta::Deleted(
              len.ok_or_else(|| de::Error::missing_field(FIELD_DELETE))? as u32,
            )),
            // If "delta_type" is "retain," construct a "Retain" TextDelta variant
            FIELD_RETAIN => Ok(TextDelta::Retain(
              len.ok_or_else(|| de::Error::missing_field(FIELD_RETAIN))? as u32,
              attrs,
            )),
            // If "delta_type" is an unknown variant, return an error
            _ => Err(de::Error::unknown_variant(
              &delta_type,
              &[FIELD_INSERT, FIELD_DELETE, FIELD_RETAIN],
            )),
          },
          // If "delta_type" is None, return an error for missing "delta type" field
          None => Err(de::Error::missing_field("delta type")),
        }
      }
    }

    // Deserialize using the TextDeltaVisitor
    deserializer.deserialize_map(TextDeltaVisitor)
  }
}
