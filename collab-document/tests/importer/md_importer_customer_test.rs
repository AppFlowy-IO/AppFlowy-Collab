use collab_document::blocks::BlockType;
use collab_document::importer::define::URL_FIELD;
use serde_json::json;

use crate::importer::util::{
  get_children_blocks, get_delta, get_delta_json, get_page_block, markdown_to_document_data,
};

#[test]
fn test_customer_unordered_list_with_link() {
  let markdown = r#"
- [The Straits Times](https://www.straitstimes.com/)
- [Channel News Asia](https://www.channelnewsasia.com/)
- [Today Online](https://www.todayonline.com/)
"#;

  let result = markdown_to_document_data(markdown);

  assert_eq!(result.blocks.len(), 4); // 1 page + 3 bulleted lists

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);
  assert_eq!(children_blocks.len(), 3);

  // - [The Straits Times](https://www.straitstimes.com/)
  assert_eq!(children_blocks[0].parent, page_block.id);
  assert_eq!(children_blocks[0].ty, "bulleted_list");
  let delta_1 = get_delta_json(&result, &children_blocks[0].id);
  assert_eq!(
    delta_1,
    json!([{"attributes":{"href":"https://www.straitstimes.com/"},"insert":"The Straits Times"}])
  );

  // - [Channel News Asia](https://www.channelnewsasia.com/)
  assert_eq!(children_blocks[1].parent, page_block.id);
  assert_eq!(children_blocks[1].ty, "bulleted_list");
  let delta_2 = get_delta_json(&result, &children_blocks[1].id);
  assert_eq!(
    delta_2,
    json!([{"attributes":{"href":"https://www.channelnewsasia.com/"},"insert":"Channel News Asia"}])
  );

  assert_eq!(children_blocks[2].parent, page_block.id);
  assert_eq!(children_blocks[2].ty, "bulleted_list");
  let delta_3 = get_delta_json(&result, &children_blocks[2].id);
  assert_eq!(
    delta_3,
    json!([{"attributes":{"href":"https://www.todayonline.com/"},"insert":"Today Online"}])
  );
}

#[test]
fn test_customer_ordered_list_with_number() {
  let markdown = r#"
1. **Ensure Dependencies**

Make sure you have the necessary packages in your `pubspec.yaml`. For example, if `FlowyText` and `AFThemeExtension` are from packages, list them under dependencies.

2. **Import Statements**

Add the necessary import statements at the top of your Dart file.

3. **Class Definition**

Here is the complete Dart file with the above steps:
"#;

  let result = markdown_to_document_data(markdown);

  assert_eq!(result.blocks.len(), 7); // 1 page + 3 ordered lists + 3 paragraphs

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);
  assert_eq!(children_blocks.len(), 6);

  // 1. **Ensure Dependencies**
  assert_eq!(children_blocks[0].parent, page_block.id);
  assert_eq!(children_blocks[0].ty, "numbered_list");
  assert_eq!(children_blocks[0].data.get("number").unwrap(), 1);

  // 2. **Import Statements**
  assert_eq!(children_blocks[2].parent, page_block.id);
  assert_eq!(children_blocks[2].ty, "numbered_list");
  assert_eq!(children_blocks[2].data.get("number").unwrap(), 2);

  // 3. **Class Definition**
  assert_eq!(children_blocks[4].parent, page_block.id);
  assert_eq!(children_blocks[4].ty, "numbered_list");
  assert_eq!(children_blocks[4].data.get("number").unwrap(), 3);
}

#[test]
fn test_customer_nested_list() {
  let markdown = r#"
- Task 1
    - Task 1 - 1
        - Task 1 - 1 - 1
    - Task 1 - 2
- Task 2
- Task 3

1. Number 1
    1. Number 1 - 1
    2. Number 1 - 2
    3. Number 1 - 3
2. Number 2
3. Number 3
"#;

  let result = markdown_to_document_data(markdown);

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  // - Task 1
  {
    assert_eq!(children_blocks[0].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[0].id),
      json!([{"insert":"Task 1"}])
    );

    // - Task 1 - 1
    //     - Task 1 - 1 - 1
    // - Task 1 - 2
    let children_blocks_1 = get_children_blocks(&result, &children_blocks[0].id);
    assert_eq!(children_blocks_1.len(), 2);

    assert_eq!(children_blocks_1[0].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_1[0].id),
      json!([{"insert":"Task 1 - 1"}])
    );

    // - Task 1 - 1 - 1
    let children_blocks_2 = get_children_blocks(&result, &children_blocks_1[0].id);
    assert_eq!(children_blocks_2.len(), 1);

    assert_eq!(children_blocks_2[0].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_2[0].id),
      json!([{"insert":"Task 1 - 1 - 1"}])
    );

    assert_eq!(children_blocks_1[1].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_1[1].id),
      json!([{"insert":"Task 1 - 2"}])
    );
  }

  // Task 2 and Task 3
  {
    assert_eq!(children_blocks[1].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[1].id),
      json!([{"insert":"Task 2"}])
    );

    assert_eq!(children_blocks[2].ty, "bulleted_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[2].id),
      json!([{"insert":"Task 3"}])
    );
  }

  // 1. Number 1
  {
    assert_eq!(children_blocks[3].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[3].id),
      json!([{"insert":"Number 1"}])
    );

    // 1. Number 1 - 1
    let children_blocks_1 = get_children_blocks(&result, &children_blocks[3].id);
    assert_eq!(children_blocks_1.len(), 3);

    assert_eq!(children_blocks_1[0].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_1[0].id),
      json!([{"insert":"Number 1 - 1"}])
    );

    // 1. Number 1 - 2
    assert_eq!(children_blocks_1[1].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_1[1].id),
      json!([{"insert":"Number 1 - 2"}])
    );

    // 1. Number 1 - 3
    assert_eq!(children_blocks_1[2].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks_1[2].id),
      json!([{"insert":"Number 1 - 3"}])
    );
  }

  // 2. Number 2
  // 3. Number 3
  {
    assert_eq!(children_blocks[4].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[4].id),
      json!([{"insert":"Number 2"}])
    );

    assert_eq!(children_blocks[5].ty, "numbered_list");
    assert_eq!(
      get_delta_json(&result, &children_blocks[5].id),
      json!([{"insert":"Number 3"}])
    );
  }
}

#[test]
fn test_customer_appflowy_editor_built_in_readme() {
  let markdown = r#"
## 👋 **Welcome to** ***[AppFlowy Editor](appflowy.io)***

AppFlowy Editor is a **highly customizable** _rich-text editor_

- [x] Customizable
- [x] Test-covered
- [ ] more to come!

|## a|_c_|
|-|-|
|**b**|d|

> Here is an example you can give a try

You can also use ***AppFlowy Editor*** as a component to build your own app.

* Use / to insert blocks
* Select text to trigger to the toolbar to format your notes.

If you have questions or feedback, please submit an issue on Github or join the community along with 1000+ builders!
"#;

  let result = markdown_to_document_data(markdown);

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  // ## 👋 **Welcome to** ***[AppFlowy Editor](appflowy.io)***
  assert_eq!(children_blocks[0].ty, "heading");
  assert_eq!(children_blocks[0].data.get("level").unwrap(), 2);
  assert_eq!(
    get_delta(&result, &children_blocks[0].id),
    r#"[{"insert":"👋 "},{"attributes":{"bold":true},"insert":"Welcome to"},{"insert":" "},{"attributes":{"bold":true,"href":"appflowy.io","italic":true},"insert":"AppFlowy Editor"}]"#
  );

  // AppFlowy Editor is a **highly customizable** _rich-text editor_
  assert_eq!(children_blocks[1].ty, "paragraph");
  assert_eq!(
    get_delta(&result, &children_blocks[1].id),
    r#"[{"insert":"AppFlowy Editor is a "},{"attributes":{"bold":true},"insert":"highly customizable"},{"insert":" "},{"attributes":{"italic":true},"insert":"rich-text editor"}]"#
  );

  // - [x] Customizable
  // - [x] Test-covered
  // - [ ] more to come!
  assert_eq!(children_blocks[2].ty, "todo_list");
  assert_eq!(children_blocks[2].data.get("checked").unwrap(), true);
  assert_eq!(
    get_delta(&result, &children_blocks[2].id),
    r#"[{"insert":"Customizable"}]"#
  );

  assert_eq!(children_blocks[3].ty, "todo_list");
  assert_eq!(children_blocks[3].data.get("checked").unwrap(), true);
  assert_eq!(
    get_delta(&result, &children_blocks[3].id),
    r#"[{"insert":"Test-covered"}]"#
  );

  assert_eq!(children_blocks[4].ty, "todo_list");
  assert_eq!(children_blocks[4].data.get("checked").unwrap(), false);
  assert_eq!(
    get_delta(&result, &children_blocks[4].id),
    r#"[{"insert":"more to come!"}]"#
  );

  //   table
  {
    /*
    |## a|_c_|
    |-|-|
    |**b**|d|
    */
    assert_eq!(children_blocks[5].ty, "simple_table");

    let rows = get_children_blocks(&result, &children_blocks[5].id);
    assert_eq!(rows.len(), 2);

    let mut cells = Vec::new();
    for row in &rows {
      let row_cells = get_children_blocks(&result, &row.id);
      cells.extend(row_cells);
    }
    assert_eq!(cells.len(), 4);

    for cell in &cells {
      println!("{:?}", cell);
    }

    for i in 0..2 {
      for j in 0..2 {
        let cell = &cells[2 * i + j];
        assert_eq!(cell.ty, "simple_table_cell");
        println!("{:?}", cell.data);
        assert_eq!(cell.data.get("colPosition").unwrap(), j);
        assert_eq!(cell.data.get("rowPosition").unwrap(), i);

        let paragraph_blocks = get_children_blocks(&result, &cell.id);
        let paragraph_block_id = paragraph_blocks[0].id.clone();
        let delta = get_delta(&result, &paragraph_block_id);
        if i == 0 && j == 0 {
          let expected_json = json!([{"insert":"## a"}]);

          assert_eq!(delta, expected_json.to_string());
        } else if i == 0 && j == 1 {
          let expected_json = json!([{"attributes":{"italic":true},"insert":"c"}]);

          assert_eq!(delta, expected_json.to_string());
        } else if i == 1 && j == 0 {
          let expected_json = json!([{"attributes":{"bold":true},"insert":"b"}]);

          assert_eq!(delta, expected_json.to_string());
        } else if i == 1 && j == 1 {
          let expected_json = json!([{"insert":"d"}]);

          assert_eq!(delta, expected_json.to_string());
        }
      }
    }
  }

  // quote
  {
    assert_eq!(children_blocks[6].ty, "quote");
    assert_eq!(
      get_delta(&result, &children_blocks[6].id),
      r#"[{"insert":"Here is an example you can give a try"}]"#
    );
  }

  assert_eq!(children_blocks[7].ty, "paragraph");
  println!("{:?}", get_delta(&result, &children_blocks[7].id));
  assert_eq!(
    get_delta(&result, &children_blocks[7].id),
    r#"[{"insert":"You can also use "},{"attributes":{"bold":true,"italic":true},"insert":"AppFlowy Editor"},{"insert":" as a component to build your own app."}]"#
  );

  // numbered list
  {
    assert_eq!(children_blocks[8].ty, "bulleted_list");
    assert_eq!(children_blocks[9].ty, "bulleted_list");
  }
}

#[test]
fn test_customer_image_in_first_level() {
  let markdown = r#"![Image](https://example.com/image.png)"#;

  let result = markdown_to_document_data(markdown);

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  for block in children_blocks.iter() {
    println!("{:?}", block);
  }

  assert_eq!(children_blocks[0].ty, "image");
  assert_eq!(
    children_blocks[0].data.get("url").unwrap(),
    "https://example.com/image.png"
  );
}

#[test]
fn test_customer_image_in_nested_level_1() {
  // notes: there's a empty line between the first nested bulleted item and the image
  let markdown = r#"
- 7/18 Consumption Spike issue
  - 7/19 Contacted Enphase and they are going to clear spike with case #16518709 - they said the update on the 18th caused the snike

    ![Untitled](Untitled.png)
  "#;

  let result = markdown_to_document_data(markdown);

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  // - First bulleted item
  let delta = get_delta(&result, &children_blocks[0].id);
  assert_eq!(children_blocks.len(), 1);
  assert_eq!(children_blocks[0].ty, BlockType::BulletedList.to_string());
  assert_eq!(delta, r#"[{"insert":"7/18 Consumption Spike issue"}]"#);

  // - First nested bulleted item
  let children_blocks_1 = get_children_blocks(&result, &children_blocks[0].id);
  let delta = get_delta(&result, &children_blocks_1[0].id);
  assert_eq!(children_blocks_1.len(), 1);
  assert_eq!(children_blocks_1[0].ty, BlockType::BulletedList.to_string());
  assert_eq!(
    delta,
    r#"[{"insert":"7/19 Contacted Enphase and they are going to clear spike with case #16518709 - they said the update on the 18th caused the snike"}]"#
  );

  // Image under the first nested bulleted item
  let children_blocks_1_1 = get_children_blocks(&result, &children_blocks_1[0].id);
  assert_eq!(children_blocks_1_1.len(), 1);
  assert_eq!(children_blocks_1_1[0].ty, BlockType::Image.to_string());
  assert_eq!(
    children_blocks_1_1[0].data.get(URL_FIELD).unwrap(),
    "Untitled.png"
  );

  let blocks = result.blocks;
  for value in blocks.iter() {
    let block = value.1;
    if block.ty == BlockType::Image.to_string() {
      assert_eq!(block.data.get(URL_FIELD).unwrap(), "Untitled.png");
    }
  }
}

#[test]
fn test_indented_image_under_paragraph() {
  let markdown = r#"
This is a paragraph 1

  ![Untitled](Untitled.png)

This is a paragraph 2
"#;

  let result = markdown_to_document_data(markdown);
  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  // First paragraph
  assert_eq!(children_blocks[0].ty, BlockType::Paragraph.to_string());
  assert_eq!(
    get_delta(&result, &children_blocks[0].id),
    r#"[{"insert":"This is a paragraph 1"}]"#
  );

  // Image
  assert_eq!(children_blocks[1].ty, BlockType::Image.to_string());
  assert_eq!(
    children_blocks[1].data.get(URL_FIELD).unwrap(),
    "Untitled.png"
  );

  // Second paragraph
  assert_eq!(children_blocks[2].ty, BlockType::Paragraph.to_string());
  assert_eq!(
    get_delta(&result, &children_blocks[2].id),
    r#"[{"insert":"This is a paragraph 2"}]"#
  );
}
