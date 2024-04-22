use assert_json_diff::assert_json_include;
use collab::preclude::Any;
use collab_folder::{timestamp, Section, SectionItem, UserId};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::util::create_folder_with_workspace;

#[tokio::test]
async fn custom_section_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;

  // By default, the folder has a favorite section
  let op = folder_test.section_op(Section::Favorite).unwrap();
  op.add_section_items(vec![SectionItem::new("1".to_string())]);

  let _ = folder_test.create_section(Section::Custom("private".to_string()));
  let op = folder_test
    .section_op(Section::Custom("private".to_string()))
    .unwrap();
  op.add_section_items(vec![SectionItem::new("2".to_string())]);

  let json = folder_test.to_json_value();
  assert_json_include!(
    actual: json,
    expected: json!({"section": {
      "favorite": {
        "1": [
          {
            "id": "1"
          }
        ]
      },
      "private": {
        "1": [
          {
            "id": "2"
          }
        ]
      }
    }})
  );
}

#[tokio::test]
async fn section_serde_test() {
  let mut data: HashMap<String, Any> = HashMap::new();
  data.insert("id".to_string(), uuid::Uuid::new_v4().to_string().into());
  data.insert("timestamp".to_string(), timestamp().into());
  let any = Any::Map(Arc::new(data));
  println!("Any: {:?}", any);
  let start = std::time::Instant::now();
  let item = SectionItem::try_from(&any).unwrap();
  let elapsed = start.elapsed();
  println!(
    "Time to convert Any to SectionItem: {:?} {:?}",
    item, elapsed
  );
}
