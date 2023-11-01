use assert_json_diff::assert_json_include;
use collab_folder::{Section, SectionItem, UserId};
use serde_json::json;

use crate::util::create_folder_with_workspace;

#[tokio::test]
async fn custom_section_test() {
  let uid = UserId::from(1);
  let folder_test = create_folder_with_workspace(uid.clone(), "w1").await;

  // By default, the folder has a favorite section
  let op = folder_test.section_op(Section::Favorite).unwrap();
  op.add_section_items(vec![SectionItem {
    id: "1".to_string(),
  }]);

  let _ = folder_test.create_section(Section::Custom("private".to_string()));
  let op = folder_test
    .section_op(Section::Custom("private".to_string()))
    .unwrap();
  op.add_section_items(vec![SectionItem {
    id: "2".to_string(),
  }]);

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
