use assert_json_diff::assert_json_include;
use collab::preclude::Any;
use collab_folder::{timestamp, Section, SectionItem, UserId};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::util::create_folder_with_workspace;

#[test]
fn custom_section_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1");

  let mut lock = folder_test.inner.lock().unwrap();
  let mut txn = lock.transact_mut();

  // By default, the folder has a favorite section
  let op = folder_test
    .section
    .section_op(&txn, Section::Favorite)
    .unwrap();
  op.add_sections_item(&mut txn, vec![SectionItem::new("1".to_string())]);

  let _ = folder_test
    .section
    .create_section(&mut txn, Section::Custom("private".to_string()));
  let op = folder_test
    .section
    .section_op(&txn, Section::Custom("private".to_string()))
    .unwrap();
  op.add_sections_item(&mut txn, vec![SectionItem::new("2".to_string())]);

  drop(txn);
  drop(lock);

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

#[test]
fn section_serde_test() {
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
