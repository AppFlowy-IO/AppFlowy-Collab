use crate::blocks::block_test_core::{BlockTestCore, generate_id};
use collab::preclude::{Attrs, Delta, YrsValue};
use collab_document::blocks::{
  BlockAction, BlockActionPayload, BlockActionType, TextDelta, deserialize_text_delta,
};

use crate::util::try_decode_from_encode_collab;
use serde_json::json;
use std::sync::Arc;

#[test]
fn insert_text_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = json!([{"insert": "Hello World"}]).to_string();
  let text_id = test.create_text(origin_delta.clone());
  let delta = test.get_text_delta_with_text_id(&text_id);
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&origin_delta).unwrap()
  );
}

#[test]
fn insert_empty_text_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = "".to_string();
  let text_id = test.create_text(origin_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let result = deserialize_text_delta(&delta);
  assert!(result.is_ok());
  assert_eq!(result.unwrap(), vec![]);
}

#[test]
fn apply_empty_delta_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = json!([{"insert": "Hello World"}]).to_string();
  let text_id = test.create_text(origin_delta);
  let origin_delta = test.get_text_delta_with_text_id(&text_id);
  let delta = "".to_string();
  test.document.apply_text_delta(&text_id, delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&origin_delta).unwrap()
  );
}
#[test]
fn format_text_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = json!([{"insert": "Hello World", "attributes": { "bold": true }}]).to_string();
  let text_id = test.create_text(origin_delta.clone());
  let delta = test.get_text_delta_with_text_id(&text_id);
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&origin_delta).unwrap()
  );
}

#[test]
fn apply_retain_delta_test() {
  let mut test = BlockTestCore::new();
  let text = "Hello World".to_string();
  let length = text.len() as u32;
  let origin_delta = json!([{"insert": "Hello World"}]).to_string();
  let text_id = test.create_text(origin_delta);
  let origin_delta = test.get_text_delta_with_text_id(&text_id);

  // retain text
  let retain_delta = json!([{ "retain": length }]).to_string();
  test.document.apply_text_delta(&text_id, retain_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&origin_delta).unwrap()
  );

  // retain text and format
  let format_delta = json!([
    {"retain": length, "attributes": { "bold": true, "italic": true }}
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, format_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!(
    [{"insert": "Hello World", "attributes": { "bold": true, "italic": true }}]
  )
  .to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );

  // retain text and clear format
  let clear_format_delta = json!([
    {"retain": length, "attributes": { "bold": null, "italic": null }}
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, clear_format_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!(
    [{"insert": "Hello World"}]
  )
  .to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
}

#[test]
fn apply_delete_delta_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = json!([{"insert": "Hello World", "attributes": { "bold": true }}]).to_string();
  let text_id = test.create_text(origin_delta);
  let delete_delta = json!([
    {"retain": 6},
    {"delete": 5},
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, delete_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([{"insert": "Hello ", "attributes": { "bold": true }}]).to_string();

  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
}

#[test]
fn apply_mark_delta_test() {
  let mut test = BlockTestCore::new();
  let origin_delta = json!([{"insert": "123456"}]).to_string();
  let text_id = test.create_text(origin_delta);
  let delta = json!([
    {"retain": 3},
    {"insert": "*"},
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, delta);

  let delta = json!([
    {"retain": 3},
    {"delete": 2},
    {"insert": "4", "attributes": { "bold": true }},
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, delta);

  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([{
      "insert": "123",
  }, {"insert": "4", "attributes": { "bold": true }}, {
      "insert": "56",
  }])
  .to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn apply_chinese_ime_delta_test() {
  let mut test = BlockTestCore::new();
  // convert zhong'wen to 中文
  let origin_delta = json!([{"insert": "z"}]).to_string();
  let text_id = test.create_text(origin_delta);
  let deltas = vec![
    json!([{"retain": 1}, {"insert": "h"}]).to_string(),
    json!([{"retain": 2}, {"insert": "o"}]).to_string(),
    json!([{"retain": 3}, {"insert": "n"}]).to_string(),
    json!([{"retain": 4}, {"insert": "g"}]).to_string(),
    json!([{"retain": 5}, {"insert": "'"}]).to_string(),
    json!([{"retain": 6}, {"insert": "w"}]).to_string(),
    json!([{"retain": 7}, {"insert": "e"}]).to_string(),
    json!([{"retain": 8}, {"insert": "n"}]).to_string(),
    json!([{"insert": "中文"}, {"delete": 9}]).to_string(),
  ];
  for delta in deltas {
    test.document.apply_text_delta(&text_id, delta);
  }
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([{"insert": "中文"}]).to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn apply_delete_chinese_delta_test() {
  let mut test = BlockTestCore::new();
  let origin_delta =
    json!([{"insert": "Hello World 嗨", "attributes": { "bold": true }}]).to_string();
  let text_id = test.create_text(origin_delta);
  let delete_delta = json!([
    {"retain": 12},
    {"delete": 1},
  ])
  .to_string();
  test.document.apply_text_delta(&text_id, delete_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([{"insert": "Hello World ", "attributes": { "bold": true }}]).to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn apply_insert_delta_test() {
  let mut test = BlockTestCore::new();
  let _text = "Hello World".to_string();
  let delta = json!([
    { "insert": "As soon as you type " },
    {
      "attributes": { "code": true, "font_color": "0xff00b5ff" },
      "insert": "/"
    },
    { "insert": " a menu will pop up. Select " },
    {
      "attributes": { "bg_color": "0x4d9c27b0" },
      "insert": "different types"
    },
    { "insert": " of content blocks you can add." }
  ])
  .to_string();
  let text_id = test.create_text(delta);
  let insert_delta = json!([{
    "retain": 1,
  }, {
    "insert": " ",
  }])
  .to_string();
  test.document.apply_text_delta(&text_id, insert_delta);
  let delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([
    { "insert": "A s soon as you type " },
    {
      "attributes": { "code": true, "font_color": "0xff00b5ff" },
      "insert": "/"
    },
    { "insert": " a menu will pop up. Select " },
    {
      "attributes": { "bg_color": "0x4d9c27b0" },
      "insert": "different types"
    },
    { "insert": " of content blocks you can add." }
  ])
  .to_string();
  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn subscribe_apply_delta_test() {
  let mut test = BlockTestCore::new();
  test.subscribe("print", |e, _| {
    println!("event: {:?}", e);
  });
  let text = "Hello World".to_string();
  let delta = json!([{ "insert": text }]).to_string();
  let text_id = test.create_text(delta);
  let delta = json!([{
    "retain": 6,
  }, {
    "insert": "World ",
  }])
  .to_string();
  test.document.apply_text_delta(&text_id, delta);
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn delta_equal_test() {
  let delta = json!([{"insert": "Hello World"}, { "retain": 6, "attributes": { "bold": true } }, { "delete": 4 } ]).to_string();
  let delta2 = json!([{"insert": "Hello World"}, { "attributes": { "bold": true }, "retain": 6 }, { "delete": 4 } ]).to_string();

  assert_eq!(
    deserialize_text_delta(&delta).unwrap(),
    deserialize_text_delta(&delta2).unwrap()
  );
}

#[test]
fn text_delta_trans_delta_test() {
  let test = BlockTestCore::new();
  let collab = &*test.document;
  let txn = collab.transact();
  let text_delta = TextDelta::Inserted("Hello World".to_string(), None);
  let delta = Delta::Inserted(YrsValue::from("Hello World"), None);
  let result = TextDelta::from(delta.clone().map(|s| s.to_string(&txn)));
  assert_eq!(result, text_delta);
  assert_eq!(result.to_delta(), Delta::insert("Hello World"));

  let attrs = Attrs::from([(Arc::from("bold"), true.into())]);
  let delta = Delta::Retain(6, Some(Box::from(attrs.clone())));
  let result = TextDelta::from(delta.clone());
  let text_delta = TextDelta::Retain(6, Some(attrs.clone()));
  assert_eq!(result, text_delta);
  assert_eq!(result.to_delta(), Delta::Retain(6, Some(Box::from(attrs))));

  let delta = Delta::Deleted(4);
  let result = TextDelta::from(delta.clone());
  let text_delta = TextDelta::Deleted(4);
  assert_eq!(result, text_delta);
  assert_eq!(result.to_delta(), Delta::delete(4));
}

#[test]
fn serialize_delta_test() {
  let delta = TextDelta::Inserted("Hello World".to_string(), None);
  let json = serde_json::to_string(&delta).unwrap();
  assert_eq!(json, r#"{"insert":"Hello World"}"#);

  let delta = TextDelta::Retain(6, Some(Attrs::from([(Arc::from("bold"), true.into())])));
  let json = serde_json::to_string(&delta).unwrap();
  assert_eq!(json, r#"{"retain":6,"attributes":{"bold":true}}"#);

  let delta = TextDelta::Deleted(4);
  let json = serde_json::to_string(&delta).unwrap();
  assert_eq!(json, r#"{"delete":4}"#);
}

#[test]
fn deserialize_delta_test() {
  let json = r#"{"insert":"Hello World"}"#;
  let delta: TextDelta = serde_json::from_str(json).unwrap();
  assert_eq!(delta, TextDelta::Inserted("Hello World".to_string(), None));

  let json = r#"{"retain":6,"attributes":{"bold":true}}"#;
  let delta: TextDelta = serde_json::from_str(json).unwrap();
  assert_eq!(
    delta,
    TextDelta::Retain(6, Some(Attrs::from([(Arc::from("bold"), true.into())])))
  );

  let json = r#"{"delete":4}"#;
  let delta: TextDelta = serde_json::from_str(json).unwrap();
  assert_eq!(delta, TextDelta::Deleted(4));

  let json = r#"{ "unexpected_field": "value" }"#;
  let result: Result<TextDelta, serde_json::Error> = serde_json::from_str(json);
  assert!(result.is_err());

  let json = r#"{}"#;
  let result: Result<TextDelta, serde_json::Error> = serde_json::from_str(json);
  assert!(result.is_err());

  let json = r#""#;
  let result: Result<TextDelta, serde_json::Error> = serde_json::from_str(json);
  assert!(result.is_err());
}

#[test]
fn apply_text_actions() {
  let mut test = BlockTestCore::new();
  let insert_delta = json!([{ "insert": "Hello " }]).to_string();
  let text_id = test.create_text(insert_delta.clone());
  let delta = json!([{
      "retain": 6,
  }, {
      "insert": "World ",
  }])
  .to_string();
  let new_text_id = generate_id();
  test.apply_action(vec![
    BlockAction {
      action: BlockActionType::ApplyTextDelta,
      payload: BlockActionPayload {
        block: None,
        prev_id: None,
        parent_id: None,
        delta: Some(delta),
        text_id: Some(text_id.clone()),
      },
    },
    BlockAction {
      action: BlockActionType::InsertText,
      payload: BlockActionPayload {
        block: None,
        prev_id: None,
        parent_id: None,
        delta: Some(insert_delta.clone()),
        text_id: Some(new_text_id.clone()),
      },
    },
  ]);
  let new_text_delta = test.get_text_delta_with_text_id(&new_text_id);
  let text_delta = test.get_text_delta_with_text_id(&text_id);
  let expect = json!([{
      "insert": "Hello World ",
  }])
  .to_string();
  assert_eq!(
    deserialize_text_delta(&new_text_delta).unwrap(),
    deserialize_text_delta(&insert_delta).unwrap()
  );
  assert_eq!(
    deserialize_text_delta(&text_delta).unwrap(),
    deserialize_text_delta(&expect).unwrap()
  );
  try_decode_from_encode_collab(&test.document);
}

#[test]
fn apply_text_actions_without_params_test() {
  let mut test = BlockTestCore::new();
  let insert_delta = json!([{ "insert": "Hello " }]).to_string();
  let text_id = test.create_text(insert_delta.clone());
  let document_data = test.get_document_data();
  test.apply_action(vec![BlockAction {
    action: BlockActionType::ApplyTextDelta,
    payload: BlockActionPayload {
      block: None,
      prev_id: None,
      parent_id: None,
      delta: None,
      text_id: Some(text_id),
    },
  }]);
  // nothing should happen
  assert_eq!(document_data, test.get_document_data());

  test.apply_action(vec![BlockAction {
    action: BlockActionType::InsertText,
    payload: BlockActionPayload {
      block: None,
      prev_id: None,
      parent_id: None,
      delta: Some(insert_delta),
      text_id: None,
    },
  }]);

  // nothing should happen
  assert_eq!(document_data, test.get_document_data());
  try_decode_from_encode_collab(&test.document);
}
