use crate::util::create_folder_with_workspace;

#[tokio::test]
async fn create_favorite_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  folder_test.add_favorites(vec!["1".to_string(), "2".to_string()]);

  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");
}
#[tokio::test]
async fn delete_favorite_test() {
  let folder_test = create_folder_with_workspace("1", "w1");
  folder_test.add_favorites(vec!["1".to_string(), "2".to_string()]);

  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 2);
  assert_eq!(favorites[0].id, "1");
  assert_eq!(favorites[1].id, "2");

  folder_test.delete_favorites(vec!["1".to_string()]);
  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 1);
  assert_eq!(favorites[0].id, "2");

  folder_test.remove_all_favorites();
  let favorites = folder_test.get_all_favorites();
  assert_eq!(favorites.len(), 0);
}
