use std::path::Path;

use fs_extra::file;
use nanoid::nanoid;
use walkdir::WalkDir;

// #[test]
// fn test_set_current_view() {
//   let uid: i64 = 185579439403307008;
//   let source = "./tests/folder_test/dbs".to_string();
//   duplicate_db(source, &uid.to_string(), |duplicate_db| {
//     let folder = create_folder_with_object_id(uid, duplicate_db);
//
//     // set current view
//     folder.set_current_view("abc");
//     let json1 = folder.to_json_value();
//     drop(folder);
//
//     // reopen
//     let folder = create_folder_with_object_id(uid, duplicate_db);
//     let json2 = folder.to_json_value();
//     assert_json_diff::assert_json_eq!(json1, json2);
//   })
// }

#[allow(dead_code)]
fn duplicate_db(source: String, folder: &str, f: impl FnOnce(&str)) {
  let dest = format!("temp/{}", nanoid!());
  let dest_path = format!("{}/{}", source, dest);
  copy_folder_recursively(&source, folder, &dest).unwrap();
  f(&dest_path);
  std::fs::remove_dir_all(dest_path).unwrap();
}

#[allow(dead_code)]
fn copy_folder_recursively(
  parent_folder: &str,
  src_folder: &str,
  dest_folder: &str,
) -> std::io::Result<()> {
  let src_path = Path::new(parent_folder).join(src_folder);
  let dest_path = Path::new(parent_folder).join(dest_folder);

  for entry in WalkDir::new(&src_path) {
    let entry = entry?;
    let entry_path = entry.path();

    let relative_entry_path = entry_path.strip_prefix(&src_path).unwrap();
    let target_path = dest_path.join(relative_entry_path);

    if entry.file_type().is_dir() {
      std::fs::create_dir_all(target_path)?;
    } else {
      let options = file::CopyOptions::new().overwrite(true);
      file::copy(entry_path, target_path, &options).unwrap();
    }
  }
  Ok(())
}
