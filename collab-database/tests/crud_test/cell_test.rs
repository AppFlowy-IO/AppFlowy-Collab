use crate::helper::create_database_with_default_data;
use collab_database::views::CreateViewParams;

#[test]
fn create_row_shared_by_two_view_test() {
  let database_test = create_database_with_default_data(1, "1");
  let params = CreateViewParams {
    id: "v1".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);

  let params = CreateViewParams {
    id: "v2".to_string(),
    ..Default::default()
  };
  database_test.create_view(params);
}
