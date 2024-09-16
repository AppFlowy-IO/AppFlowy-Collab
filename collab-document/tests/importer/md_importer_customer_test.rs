use serde_json::json;

use crate::importer::util::{
  dump_page_blocks, get_children_blocks, get_delta, get_delta_json, get_page_block,
  markdown_to_document_data,
};
use serde_json::json;

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
- Task Parent One
    - Task One + Parent
        - Task Two
    - Task Three
- Task Four
- Task Five

1. Numbered List
    1. Which
    2. Is
    3. Nested
2. Back to top level
"#;

  let result = markdown_to_document_data(markdown);

  let page_block = get_page_block(&result);
  let children_blocks = get_children_blocks(&result, &page_block.id);

  // TODO: This test failed.
  for (idx, block) in children_blocks.iter().enumerate() {
    println!("block: {:?}", block);
  }

  for text in result.meta.text_map.iter() {
    println!("text: {:?}", text);
  }

  // - Task Parent One
  {
    assert_eq!(children_blocks[0].ty, "bulleted_list");

    let children_blocks_1 = get_children_blocks(&result, &children_blocks[0].id);
    assert_eq!(children_blocks_1.len(), 2);

    // - Task One + Parent
    assert_eq!(children_blocks_1[0].ty, "bulleted_list");
    let children_blocks_2 = get_children_blocks(&result, &children_blocks_1[0].id);
    assert_eq!(children_blocks_2.len(), 1);
    assert_eq!(
      get_delta(&result, &children_blocks_2[0].id),
      r#"[{"insert":"Task One + Parent"}]"#
    );

    // - Task Two
    assert_eq!(children_blocks_2[0].ty, "bulleted_list");
    assert_eq!(
      get_delta(&result, &children_blocks_2[0].id),
      r#"[{"insert":"Task Two"}]"#
    );

    // - Task Three
    assert_eq!(children_blocks_2[0].ty, "bulleted_list");
    assert_eq!(
      get_delta(&result, &children_blocks_2[0].id),
      r#"[{"insert":"Task Three"}]"#
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

  for block in &children_blocks {
    println!("{:?}", block);
    // println!("{:?}", get_delta(&result, &block.id));
  }

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
    assert_eq!(children_blocks[5].ty, "table");
    assert_eq!(children_blocks[5].data.get("colsLen").unwrap(), 2);
    assert_eq!(children_blocks[5].data.get("rowsLen").unwrap(), 2);

    let cells = get_children_blocks(&result, &children_blocks[5].id);
    assert_eq!(cells.len(), 4);

    for cell in &cells {
      println!("{:?}", cell);
    }

    for i in 0..2 {
      for j in 0..2 {
        let cell = &cells[2 * i + j];
        assert_eq!(cell.ty, "table/cell");
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
