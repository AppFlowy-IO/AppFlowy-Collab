use crate::importer::util::{get_children_blocks, get_page_block, markdown_to_document_data};
use collab_document::block_parser::{BlockParser, OutputFormat, ParseContext};
use collab_document::block_parser::{DocumentParser, parsers::*};
use collab_document::blocks::BlockType;

#[test]
fn test_desktop_guide_markdown_parser() {
  let desktop_guide_markdown = r#"# AppFlowy Desktop on macOS, Windows, and Linux
[Download](https://appflowy.io/download)
### Basics
- [ ] Click anywhere and just start typing.
- [ ] Highlight any text, and use the editing menu to _style_ **your** <u>writing</u> `however` you ~~like.~~
- [ ] As soon as you type `/` a menu will pop up. Select different types of content blocks you can add.
	- [ ] Type `/` followed by `/bullet` or `/num` to create a list.
- [x] Click `+ New Page `button at the top of your sidebar to quickly add a new page.
- [ ] Click `+` next to any page title or space name in the sidebar to add a new page/subpage:
	* Document
	* Grid
	* Kanban Board
	* Calendar
	* AI Chat
	* or through import

---
### Keyboard shortcuts, markdown, and code block
1. Keyboard shortcuts [guide](https://appflowy.gitbook.io/docs/essential-documentation/shortcuts)
1. Markdown [reference](https://appflowy.gitbook.io/docs/essential-documentation/markdown)
1. Type `/code` to insert a code block
```rust
// This is the main function.
fn main() {
    // Print text to the console.
    println!("Hello World!");
}
```

---

### Spaces
Create multiple spaces to better organize your work
![](https://github.com/AppFlowy-IO/AppFlowy/blob/main/doc/readme/desktop_guide_1.jpg?raw=true)
![](https://github.com/AppFlowy-IO/AppFlowy/blob/main/doc/readme/desktop_guide_2.jpg?raw=true)
---

## Have a question❓
> Click `?` at the bottom right for help and support."#;

  let document_data = markdown_to_document_data(desktop_guide_markdown);

  println!("Total blocks: {}", document_data.blocks.len());
  let page_block = get_page_block(&document_data);
  let children_blocks = get_children_blocks(&document_data, &page_block.id);
  println!("Page children count: {}", children_blocks.len());

  // Debug: Print all blocks and their data
  println!("=== All Blocks ===");
  for (id, block) in &document_data.blocks {
    println!(
      "Block ID: {}, Type: {}, Parent: {}, Data: {:?}",
      id, block.ty, block.parent, block.data
    );
  }

  // Debug: Look for image blocks specifically
  println!("=== Image Blocks ===");
  for (id, block) in &document_data.blocks {
    if block.ty == "image" {
      println!(
        "Image Block ID: {}, Parent: {}, Data: {:?}",
        id, block.parent, block.data
      );
    }
  }

  // Debug: Print children map
  println!("=== Children Map ===");
  for (parent_id, children) in &document_data.meta.children_map {
    println!("Parent: {} -> Children: {:?}", parent_id, children);
  }

  // Debug: Check if image blocks are in the page children
  println!("=== Page Children Details ===");
  for child in children_blocks {
    println!(
      "Child Block: ID={}, Type={}, Parent={}",
      child.id, child.ty, child.parent
    );
  }

  let parser = DocumentParser::with_default_parsers();

  let result = parser
    .parse_document(&document_data, OutputFormat::Markdown)
    .unwrap()
    .split("\n")
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
  for line in result {
    println!("{}", line);
  }

  println!("--------------------------------");
  let result = parser
    .parse_document(&document_data, OutputFormat::PlainText)
    .unwrap()
    .split("\n")
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
  for line in result {
    println!("{}", line);
  }
}
