use crate::database_test::helper::create_database_with_default_data;
use assert_json_diff::assert_json_eq;
use collab::core::collab::CollabOptions;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;

#[tokio::test]
async fn encode_database_collab_test() {
  let database_id = uuid::Uuid::new_v4().to_string();
  let database_test = create_database_with_default_data(1, &database_id).await;

  let database_collab = database_test.encode_database_collabs().await.unwrap();
  assert_eq!(database_collab.encoded_row_collabs.len(), 3);

  for (index, encoded_info) in database_collab.encoded_row_collabs.into_iter().enumerate() {
    let object_id = database_test.pre_define_row_ids[index].clone();
    let options = CollabOptions::new(object_id.to_string(), database_test.client_id)
      .with_data_source(encoded_info.encoded_collab.into());
    let collab = Collab::new_with_options(CollabOrigin::Empty, options).unwrap();
    let json = collab.to_json_value();
    let expected_json = database_test
      .get_database_row(&object_id)
      .await
      .unwrap()
      .read()
      .await
      .to_json_value();
    assert_json_eq!(json, expected_json);
  }
}
