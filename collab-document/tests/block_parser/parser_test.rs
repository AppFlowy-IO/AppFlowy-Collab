use crate::importer::util::markdown_to_document_data;
use collab_document::block_parser::{DocumentParser, OutputFormat};

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

  let expected_result = r#"AppFlowy Desktop on macOS, Windows, and Linux
Download
Basics
Click anywhere and just start typing.
Highlight any text, and use the editing menu to style your writing however you like.
<u>
</u>

As soon as you type / a menu will pop up. Select different types of content blocks you can add.
  Type / followed by /bullet or /num to create a list.

Click + New Page button at the top of your sidebar to quickly add a new page.
Click + next to any page title or space name in the sidebar to add a new page/subpage:
  Document
  Grid
  Kanban Board
  Calendar
  AI Chat
  or through import

---
Keyboard shortcuts, markdown, and code block
1. Keyboard shortcuts guide
1. Markdown reference
1. Type /code to insert a code block
// This is the main function.
fn main() {
    // Print text to the console.
    println!("Hello World!");
}
---
Spaces
Create multiple spaces to better organize your work


https://github.com/AppFlowy-IO/AppFlowy/blob/main/doc/readme/desktop_guide_1.jpg?raw=true
https://github.com/AppFlowy-IO/AppFlowy/blob/main/doc/readme/desktop_guide_2.jpg?raw=true

Have a question❓
Click ? at the bottom right for help and support."#;

  let document_data = markdown_to_document_data(desktop_guide_markdown);
  let parser = DocumentParser::with_default_parsers();
  let result = parser.parse_document(&document_data, OutputFormat::PlainText);
  assert_eq!(result.unwrap(), expected_result);
}

#[test]
fn test_table_markdown_parser() {
  let table_markdown = r#"# Table Examples
## Simple Table
| Company | Type | City |
|------|-----|------|
| AppFlowy 1 | 1  | [NYC](https://appflowy.io)  |
| AppFlowy 2 | 2  | **LA**   |
| AppFlowy 3 | 3  | `Chicago` |"#;

  let expected_result = r#"Table Examples
Simple Table
Company	Type	City
AppFlowy 1	1	NYC
AppFlowy 2	2	LA
AppFlowy 3	3	Chicago"#;

  let document_data = markdown_to_document_data(table_markdown);
  let parser = DocumentParser::with_default_parsers();
  let result = parser.parse_document(&document_data, OutputFormat::PlainText);
  assert_eq!(result.unwrap(), expected_result);

  let result = parser
    .parse_document(&document_data, OutputFormat::Markdown)
    .unwrap();

  let expected_markdown = r#"# Table Examples
## Simple Table
| Company | Type | City |
|------|------|------|
| AppFlowy 1 | 1 | [NYC](https://appflowy.io) |
| AppFlowy 2 | 2 | **LA** |
| AppFlowy 3 | 3 | `Chicago` |"#;

  assert_eq!(result, expected_markdown);
}
