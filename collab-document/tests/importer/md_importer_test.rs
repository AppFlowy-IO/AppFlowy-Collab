use collab_document::blocks::DocumentData;
use collab_document::importer::md_importer::{insert_text_to_delta, MDImporter};
use serde_json::{json, Value};
use std::collections::HashMap;

fn markdown_to_document_data(md: &str) -> DocumentData {
  let importer = MDImporter::new();
  let result = importer.import("test_document", md);
  result.unwrap()
}
fn parse_json(s: &str) -> Value {
  serde_json::from_str(s).unwrap()
}

#[test]
fn test_simple_paragraph() {
  let markdown = "Hello, world!";
  let result = markdown_to_document_data(markdown);

  assert_eq!(result.blocks.len(), 2); // root and paragraph
  assert!(result.blocks.values().any(|b| b.ty == "page"));
  assert!(result.blocks.values().any(|b| b.ty == "paragraph"));

  let paragraph = result
    .blocks
    .values()
    .find(|b| b.ty == "paragraph")
    .unwrap();
  let delta = result
    .meta
    .text_map
    .as_ref()
    .unwrap()
    .get(&paragraph.id)
    .unwrap();

  let expected = insert_text_to_delta(None, "Hello, world!".to_string(), HashMap::new());
  assert_eq!(delta, &expected.to_string());
}

#[test]
fn test_inline_elements() {
  let markdown = "This is **bold**, *italic*, ~~delete~~, and [a link](https://example.com).";

  let result = markdown_to_document_data(markdown);

  assert_eq!(result.blocks.len(), 2); // root 和 paragraph

  let paragraph = result
    .blocks
    .values()
    .find(|b| b.ty == "paragraph")
    .unwrap();

  let text_map = result.meta.text_map.as_ref().unwrap();
  let delta_json = parse_json(text_map.get(&paragraph.id).unwrap());

  let expected_delta = json!([
      {"insert": "This is "},
      {"insert": "bold", "attributes": {"bold": true}},
      {"insert": ", "},
      {"insert": "italic", "attributes": {"italic": true}},
      {"insert": ", "},
      {"insert": "delete", "attributes": {"strikethrough": true}},
      {"insert": ", and "},
      {"insert": "a link", "attributes": {"href": "https://example.com"}},
      {"insert": "."}
  ]);

  assert_eq!(delta_json, expected_delta);
}

#[test]
fn test_inline_math() {
  let markdown = "This is an inline math formula: $E=mc^2$.";

  let result = markdown_to_document_data(markdown);

  let paragraph = result
    .blocks
    .values()
    .find(|b| b.ty == "paragraph")
    .unwrap();

  let text_map = result.meta.text_map.as_ref().unwrap();
  let delta_json = parse_json(text_map.get(&paragraph.id).unwrap());

  let expected_delta = json!([
      {"insert": "This is an inline math formula: "},
      {"insert": "$", "attributes": {"formula": "E=mc^2"}},
      {"insert": "."}
  ]);

  assert_eq!(delta_json, expected_delta);
}
#[test]
fn test_mixed_inline_elements() {
  let markdown = "This is ***bold and italic*** and `code`.";

  let result = markdown_to_document_data(markdown);

  let paragraph = result
    .blocks
    .values()
    .find(|b| b.ty == "paragraph")
    .unwrap();

  let text_map = result.meta.text_map.as_ref().unwrap();
  let delta_json = parse_json(text_map.get(&paragraph.id).unwrap());

  let expected_delta = json!([
      {"insert": "This is "},
      {"insert": "bold and italic", "attributes": {"bold": true, "italic": true}},
      {"insert": " and "},
      {"insert": "code", "attributes": {"code": true}},
      {"insert": "."}
  ]);

  assert_eq!(delta_json, expected_delta);
}

#[test]
fn test_nested_inline_elements() {
  let markdown = "This is **bold with *nested italic* text**.";

  let result = markdown_to_document_data(markdown);

  let paragraph = result
    .blocks
    .values()
    .find(|b| b.ty == "paragraph")
    .unwrap();

  let text_map = result.meta.text_map.as_ref().unwrap();
  let delta_json = parse_json(text_map.get(&paragraph.id).unwrap());

  let expected_delta = json!([
      {"insert": "This is "},
      {"insert": "bold with ", "attributes": {"bold": true}},
      {"insert": "nested italic", "attributes": {"bold": true, "italic": true}},
      {"insert": " text", "attributes": {"bold": true}},
      {"insert": "."}
  ]);

  assert_eq!(delta_json, expected_delta);
}

#[test]
fn test_headings() {
  let markdown =
    "# Heading 1\n## Heading 2\n### Heading 3\n#### Heading 4\n##### Heading 5\n###### Heading 6";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();
  let headings: Vec<_> = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect();

  assert_eq!(headings.len(), 6);
  assert_eq!(headings[0].data["level"], 1);
  assert_eq!(headings[1].data["level"], 2);
  assert_eq!(headings[2].data["level"], 3);
  assert_eq!(headings[3].data["level"], 4);
  assert_eq!(headings[4].data["level"], 5);
  assert_eq!(headings[5].data["level"], 6);

  for (i, heading) in headings.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&heading.id).unwrap());
    let expected_delta = json!([
        {"insert": format!("Heading {}", i + 1)}
    ]);
    assert_eq!(delta_json, expected_delta);

    let ty = heading.ty.clone();

    assert_eq!(ty, "heading");
  }
}

#[test]
fn test_numbered_list() {
  let markdown = "1. First item\n2. Second item\n3. Third item";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();

  let list = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect::<Vec<_>>();

  assert_eq!(list.len(), 3);

  for (i, item) in list.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&item.id).unwrap());
    let expected_delta = json!([
        {"insert": format!("{} item", ["First", "Second", "Third"][i])}
    ]);
    assert_eq!(delta_json, expected_delta);

    let ty = item.ty.clone();

    assert_eq!(ty, "numbered_list");
  }
}

#[test]
fn test_bulleted_list() {
  let markdown = "* First item\n- Second item\n* Third item";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();

  let list = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect::<Vec<_>>();

  assert_eq!(list.len(), 3);

  for (i, item) in list.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&item.id).unwrap());
    let expected_delta = json!([
        {"insert": format!("{} item", ["First", "Second", "Third"][i])}
    ]);
    assert_eq!(delta_json, expected_delta);

    let ty = item.ty.clone();

    assert_eq!(ty, "bulleted_list");
  }
}

#[test]
fn test_checkbox() {
  let markdown = "- [ ] Unchecked\n- [x] Checked";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();

  let list = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect::<Vec<_>>();

  assert_eq!(list.len(), 2);

  for (i, item) in list.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&item.id).unwrap());
    let expected_delta = json!([
        {"insert": format!("{}", ["Unchecked", "Checked"][i])}
    ]);
    assert_eq!(delta_json, expected_delta);

    let data = item.data.clone();

    let is_checked = data
      .get("checked")
      .and_then(|v| v.as_bool())
      .unwrap_or(false);

    assert_eq!(is_checked, i != 0);

    let ty = item.ty.clone();

    assert_eq!(ty, "todo_list");
  }
}

#[test]
fn test_mix_list() {
  let markdown = "1. First item\n- Second item\n3. Third item\n- [ ] Fourth item";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();

  let list = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect::<Vec<_>>();

  assert_eq!(list.len(), 4);

  for (i, item) in list.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&item.id).unwrap());
    let expected_delta = json!([
        {"insert": format!("{} item", ["First", "Second", "Third", "Fourth"][i])}
    ]);
    assert_eq!(delta_json, expected_delta);

    let data = item.data.clone();
    let ty = item.ty.clone();

    if i == 0 {
      assert_eq!(ty, "numbered_list");
    } else if i == 1 {
      assert_eq!(ty, "bulleted_list");
    } else if i == 2 {
      assert_eq!(ty, "numbered_list");
    }

    if i == 3 {
      assert_eq!(ty, "todo_list");
      assert!(!data
        .get("checked")
        .and_then(|v| v.as_bool())
        .expect("'checked' should be a boolean value"));
    }
  }
}

#[test]
fn test_quote_list() {
  let markdown = "> First item\nThis is a paragraph\n\n> Second item\n\n> Third item";

  let result = markdown_to_document_data(markdown);
  let page = result.blocks.get("test_document").unwrap();

  let list = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .collect::<Vec<_>>();

  assert_eq!(list.len(), 3);

  for (i, item) in list.iter().enumerate() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let delta_json = parse_json(text_map.get(&item.id).unwrap());

    let ty = item.ty.clone();
    assert_eq!(ty, "quote");

    if i == 0 {
      let expected_delta = json!([
          {"insert": "First item\nThis is a paragraph"}
      ]);
      assert_eq!(delta_json, expected_delta);
    } else {
      let expected_delta = json!([
          {"insert": format!("{} item", ["Second", "Third"][i - 1])}
      ]);
      assert_eq!(delta_json, expected_delta);
    }
  }
}

#[test]
fn test_code_block() {
  let markdown = "```\nfn main() {\n    println!(\"Hello, world!\");\n}\n```";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();
  let code_block = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .next()
    .unwrap();

  let text_map = result.meta.text_map.as_ref().unwrap();
  let delta_json = parse_json(text_map.get(&code_block.id).unwrap());

  let expected_delta = json!([
      {"insert": "fn main() {\n    println!(\"Hello, world!\");\n}"}
  ]);

  assert_eq!(delta_json, expected_delta);

  let ty = code_block.ty.clone();

  assert_eq!(ty, "code");

  let language = code_block.data.get("language").unwrap().as_str().unwrap();

  assert_eq!(language, "");
}

#[test]
fn test_divider() {
  let markdown = "---";

  let result = markdown_to_document_data(markdown);

  let page = result.blocks.get("test_document").unwrap();
  let divider = result
    .meta
    .children_map
    .get(&page.id)
    .unwrap()
    .iter()
    .map(|id| result.blocks.get(id).unwrap())
    .next()
    .unwrap();

  let ty = divider.ty.clone();

  assert_eq!(ty, "divider");
}

#[test]
fn test_image() {
  let markdown = "![Alt text](https://example.com/image.png \"Image title\")";

  let result = markdown_to_document_data(markdown);

  let image = result.blocks.values().find(|b| b.ty == "image").unwrap();

  let data = image.data.clone();

  let ty = image.ty.clone();

  assert_eq!(ty, "image");

  let src = data.get("url").unwrap().as_str().unwrap();

  assert_eq!(src, "https://example.com/image.png");

  let image_type = data.get("image_type").unwrap().to_string();

  assert_eq!(image_type, "2".to_string());
}

#[test]
fn test_math() {
  let markdown = "$$\nE=mc^2\n$$";
  let result = markdown_to_document_data(markdown);

  let math = result
    .blocks
    .values()
    .find(|b| b.ty == "math_equation")
    .unwrap();

  let data = math.data.clone();

  let ty = math.ty.clone();
  assert_eq!(ty, "math_equation");

  let formula = data.get("formula").unwrap().as_str().unwrap();

  assert_eq!(formula, "E=mc^2");
}

#[test]
fn test_link_reference() {
  let markdown = "[link]: https://example.com";

  let result = markdown_to_document_data(markdown);

  let link_preview = result
    .blocks
    .values()
    .find(|b| b.ty == "link_preview")
    .unwrap();

  let data = link_preview.data.clone();

  let ty = link_preview.ty.clone();

  assert_eq!(ty, "link_preview");

  let url = data.get("url").unwrap().as_str().unwrap();

  assert_eq!(url, "https://example.com");
}

#[test]
fn test_image_reference() {
  let markdown = "[image]: https://example.com/image.png";

  let result = markdown_to_document_data(markdown);

  let image = result.blocks.values().find(|b| b.ty == "image").unwrap();

  let data = image.data.clone();

  let ty = image.ty.clone();

  assert_eq!(ty, "image");

  let src = data.get("url").unwrap().as_str().unwrap();

  assert_eq!(src, "https://example.com/image.png");

  let image_type = data.get("image_type").unwrap().to_string();

  assert_eq!(image_type, "2".to_string());
}
#[test]
fn test_table() {
  let markdown = "| Header 1 | Header 2 | Header 3 |\n| --- | --- | --- |\n| Row 1, Col 0 | Row 1, Col 1 | Row 1, Col 2 |\n| Row 2, Col 0 | Row 2, Col 1 | Row 2, Col 2 |";

  let result = markdown_to_document_data(markdown);

  let table = result.blocks.values().find(|b| b.ty == "table").unwrap();

  let data = table.data.clone();

  let ty = table.ty.clone();

  assert_eq!(ty, "table");

  assert_eq!(data.get("rowsLen").unwrap().as_u64().unwrap(), 3);
  assert_eq!(data.get("colsLen").unwrap().as_u64().unwrap(), 3);

  let table_cells = result
    .blocks
    .values()
    .filter(|b| b.ty == "table/cell")
    .collect::<Vec<_>>();

  assert_eq!(table_cells.len(), 9);

  for cell in table_cells.iter() {
    let text_map = result.meta.text_map.as_ref().unwrap();
    let paragraph_block = result
      .meta
      .children_map
      .get(&cell.id)
      .unwrap()
      .iter()
      .map(|id| result.blocks.get(id).unwrap())
      .next()
      .unwrap();
    let delta_json = parse_json(text_map.get(&paragraph_block.id).unwrap());
    let data = cell.data.clone();
    let row_position = data.get("rowPosition").unwrap().as_u64().unwrap();
    let col_position = data.get("colPosition").unwrap().as_u64().unwrap();

    if row_position == 0 {
      let expected_delta = json!([
          {"insert": format!("Header {}", col_position + 1)}
      ]);
      assert_eq!(delta_json, expected_delta);
    } else {
      let expected_delta = json!([
          {"insert": format!("Row {}, Col {}", row_position, col_position)}
      ]);
      assert_eq!(delta_json, expected_delta);
    }
  }
}
