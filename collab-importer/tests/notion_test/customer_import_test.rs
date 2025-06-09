use crate::util::sync_unzip_asset;
use collab_document::blocks::BlockType;
use collab_importer::notion::NotionImporter;

/// Customer import test 1
///
/// There's a database in the zip file and a row page in the database, and the content in the row page has a heading at the beginning.
///
/// Page Structure:
/// |- disk (database)
///     |- Passthrough disk (document in row page)
#[tokio::test]
async fn test_customer_import_1() {
  let (_cleaner, file_path) = sync_unzip_asset("row_page_with_headings").await.unwrap();
  let importer = NotionImporter::new(
    1,
    &file_path,
    uuid::Uuid::new_v4(),
    "http://test.appflowy.cloud".to_string(),
  )
  .unwrap();
  let info = importer.import().await.unwrap();
  let views = info.views();

  for view in views {
    println!("{}", view.notion_name);
  }

  let disk = views
    .iter()
    .find(|view| view.notion_name == "Disk")
    .unwrap()
    .as_database()
    .await
    .unwrap();

  let passthrough_disk = disk
    .row_documents
    .iter()
    .find(|row| row.page.notion_name == "Passthrough disk")
    .unwrap()
    .page
    .as_document()
    .await
    .unwrap();

  // Sample:
  // # Passthrough disk
  //
  // Status: Not started
  //
  // # Purpose
  // ---
  //
  // How to passthrough a physical disk to a VM
  //
  // ## How-To
  // ---
  let document = passthrough_disk.0;
  let blocks = document.get_document_data().unwrap().blocks;

  println!("{:?}", blocks);

  // 2 heading blocks, the first one is the title of the page
  let first_h1 = blocks
    .values()
    .filter(|block| {
      block.ty == BlockType::Heading.as_str() && block.data.get("level").unwrap() == 1
    })
    .collect::<Vec<_>>();
  assert_eq!(first_h1.len(), 1);
  // check the text in the heading blocks
  let text_in_first_heading = document.get_plain_text_from_block(&first_h1[0].id).unwrap();
  assert_eq!(text_in_first_heading, "Purpose");
  let first_h2 = blocks
    .values()
    .filter(|block| {
      block.ty == BlockType::Heading.as_str() && block.data.get("level").unwrap() == 2
    })
    .collect::<Vec<_>>();
  assert_eq!(first_h2.len(), 1);
  let text_in_second_heading = document.get_plain_text_from_block(&first_h2[0].id).unwrap();
  assert_eq!(text_in_second_heading, "How-To");

  // 2 divider blocks
  let divider_blocks = blocks
    .values()
    .filter(|block| block.ty == BlockType::Divider.as_str())
    .collect::<Vec<_>>();
  assert_eq!(divider_blocks.len(), 2);

  // 1 paragraph blocks, the content between the first heading and the second empty line will be removed
  let paragraph_blocks = blocks
    .values()
    .filter(|block| block.ty == BlockType::Paragraph.as_str())
    .collect::<Vec<_>>();
  assert_eq!(paragraph_blocks.len(), 1);
  // check the text in the paragraph block
  let text_in_paragraph = document
    .get_plain_text_from_block(&paragraph_blocks[0].id)
    .unwrap();
  assert_eq!(
    text_in_paragraph,
    "How to passthrough a physical disk to a VM"
  );
}
