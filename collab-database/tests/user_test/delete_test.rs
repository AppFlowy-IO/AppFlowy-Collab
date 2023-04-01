use crate::helper::create_user_database;
use collab_database::views::CreateViewParams;

#[test]
fn delete_database_inline_view_test() {
  let user_db = create_user_database(1);
  let database = user_db
    .create_database(
      "d1",
      CreateViewParams {
        id: "v1".to_string(),
        ..Default::default()
      },
    )
    .unwrap();

  for i in 2..5 {
    database.create_view(CreateViewParams {
      id: format!("v{}", i),
      ..Default::default()
    });
  }

  let views = database.views.get_all_views();
  assert_eq!(views.len(), 4);

  user_db.delete_view("d1", "v1");
  let views = database.views.get_all_views();
  assert_eq!(views.len(), 0);
}
