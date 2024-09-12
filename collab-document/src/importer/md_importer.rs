use crate::blocks::{Block, DocumentData, DocumentMeta};
use crate::document_data::generate_id;
use markdown::mdast::AlignKind;
use markdown::{mdast, message, to_mdast, Constructs, ParseOptions};
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct MDImporter {}

impl Default for MDImporter {
  fn default() -> Self {
    Self::new()
  }
}

impl MDImporter {
  pub fn new() -> Self {
    Self {}
  }

  pub fn import(&self, document_id: &str, md: &str) -> Result<DocumentData, message::Message> {
    let mdast = to_mdast(
      md,
      &ParseOptions {
        gfm_strikethrough_single_tilde: true,
        constructs: Constructs {
          math_text: true,
          math_flow: true,
          autolink: true,
          ..Constructs::gfm()
        },
        ..ParseOptions::gfm()
      },
    )?;
    let mut document_data = DocumentData {
      page_id: document_id.to_string(),
      blocks: HashMap::new(),
      meta: DocumentMeta {
        children_map: HashMap::new(),
        text_map: Some(HashMap::new()),
      },
    };

    process_node(
      &mut document_data,
      &mdast,
      None,
      Some(document_id.to_string()),
      None,
    );

    Ok(document_data)
  }
}

fn process_node(
  document_data: &mut DocumentData,
  node: &mdast::Node,
  parent_id: Option<String>,
  block_id: Option<String>,
  list_type: Option<&str>,
) {
  if is_inline_node(node) {
    process_inline(document_data, node, parent_id);
    return;
  }

  if let Some((children, list_type)) = get_list_info(node) {
    process_children(document_data, children, parent_id, Some(list_type));
    return;
  }

  let id = block_id.unwrap_or_else(generate_id);

  let block = create_block(&id, node, parent_id.clone(), list_type);

  document_data.blocks.insert(id.clone(), block);

  update_children_map(document_data, parent_id, &id);

  match node {
    mdast::Node::Root(root) => {
      process_children(document_data, &root.children, Some(id.clone()), None)
    },
    mdast::Node::Paragraph(para) => {
      process_children(document_data, &para.children, Some(id.clone()), None)
    },
    mdast::Node::Heading(heading) => {
      process_children(document_data, &heading.children, Some(id.clone()), None)
    },
    mdast::Node::BlockQuote(_) | mdast::Node::ListItem(_) => {
      if let Some(mdast::Node::Paragraph(para)) = node_children(node).and_then(|c| c.first()) {
        process_children(document_data, &para.children, Some(id.clone()), None);
      }
    },
    mdast::Node::Code(code) => {
      let mut delta = Delta::new();
      delta.insert(code.value.clone(), Vec::new());
      insert_delta_to_text_map(document_data, &id, delta);
    },
    mdast::Node::Table(table) => process_table(document_data, table, &id),
    _ => {},
  }
}

fn node_type_to_string(node: &mdast::Node, list_type: Option<&str>) -> String {
  match node {
    mdast::Node::Root(_) => "page",
    mdast::Node::Paragraph(_) => "paragraph",
    mdast::Node::Heading(_) => "heading",
    mdast::Node::BlockQuote(_) => "quote",
    mdast::Node::ListItem(list) => {
      if list.checked.is_some() {
        "todo_list"
      } else {
        match list_type {
          Some("numbered_list") => "numbered_list",
          Some("bulleted_list") => "bulleted_list",
          _ => "bulleted_list",
        }
      }
    },
    mdast::Node::Definition(defi) => {
      let url = defi.url.to_string();
      if is_image_url(&url) {
        "image"
      } else {
        "link_preview"
      }
    },
    mdast::Node::Code(_) => "code",
    mdast::Node::Image(_) => "image",
    mdast::Node::Math(_) => "math_equation",
    mdast::Node::ThematicBreak(_) => "divider",
    mdast::Node::Table(_) => "table",
    mdast::Node::TableCell(_) => "table/cell",
    _ => "paragraph",
  }
  .to_string()
}

fn is_image_url(url: &str) -> bool {
  ["png", "jpg", "jpeg", "gif", "svg", "webp"]
    .iter()
    .any(|ext| url.ends_with(ext))
}

fn node_to_data(node: &mdast::Node) -> HashMap<String, Value> {
  let mut data = HashMap::new();
  match node {
    mdast::Node::Heading(heading) => {
      data.insert("level".to_string(), Value::Number(heading.depth.into()));
    },
    mdast::Node::Code(code) => {
      let language = code.lang.as_ref().cloned().unwrap_or_default();
      data.insert("language".to_string(), Value::String(language));
    },
    mdast::Node::Image(image) => {
      data.insert("url".to_string(), Value::String(image.url.clone()));
      data.insert("image_type".to_string(), 2.into());
    },
    mdast::Node::Math(math) => {
      data.insert("formula".to_string(), Value::String(math.value.clone()));
    },
    mdast::Node::Table(table) => {
      let rows_len = table.children.len();
      data.insert("rowsLen".to_string(), rows_len.into());
      data.insert("colDefaultWidth".to_string(), 150.into());
      data.insert("rowDefaultHeight".to_string(), 37.into());
      let cols_len = table
        .children
        .first()
        .map_or(0, |row| row.children().map(|c| c.len()).unwrap_or(0));
      data.insert("colsLen".to_string(), cols_len.into());
    },

    mdast::Node::ListItem(list) => {
      if let Some(checked) = list.checked {
        data.insert("checked".to_string(), Value::Bool(checked));
      }
    },
    mdast::Node::Definition(defi) => {
      let url = defi.url.to_string();
      if is_image_url(&url) {
        data.insert("image_type".to_string(), 2.into());
      }
      data.insert("url".to_string(), Value::String(url));
    },
    _ => {},
  }
  data
}

fn is_inline_node(node: &mdast::Node) -> bool {
  matches!(
    node,
    mdast::Node::Text(_)
      | mdast::Node::Strong(_)
      | mdast::Node::Emphasis(_)
      | mdast::Node::Link(_)
      | mdast::Node::InlineCode(_)
      | mdast::Node::InlineMath(_)
      | mdast::Node::Delete(_)
  )
}

fn process_inline(document_data: &mut DocumentData, node: &mdast::Node, parent_id: Option<String>) {
  if let Some(parent_id) = parent_id {
    let delta = process_inline_node(node, Vec::new());
    insert_delta_to_text_map(document_data, &parent_id, delta);
  }
}

fn get_list_info(node: &mdast::Node) -> Option<(&Vec<mdast::Node>, &'static str)> {
  if let mdast::Node::List(list) = node {
    let list_type = if list.ordered {
      "numbered_list"
    } else {
      "bulleted_list"
    };
    Some((&list.children, list_type))
  } else {
    None
  }
}

fn create_block(
  id: &str,
  node: &mdast::Node,
  parent_id: Option<String>,
  list_type: Option<&str>,
) -> Block {
  Block {
    id: id.to_string(),
    ty: node_type_to_string(node, list_type),
    data: node_to_data(node),
    parent: parent_id.unwrap_or_default(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some("text".to_string()),
  }
}

fn update_children_map(
  document_data: &mut DocumentData,
  parent_id: Option<String>,
  child_id: &str,
) {
  if let Some(parent) = parent_id {
    document_data
      .meta
      .children_map
      .entry(parent)
      .or_default()
      .push(child_id.to_string());
  }
}

fn node_children(node: &mdast::Node) -> Option<&Vec<mdast::Node>> {
  match node {
    mdast::Node::BlockQuote(quote) => Some(&quote.children),
    mdast::Node::ListItem(list) => Some(&list.children),
    _ => None,
  }
}

fn process_table(document_data: &mut DocumentData, table: &mdast::Table, parent_id: &str) {
  for (row_index, row) in table.children.iter().enumerate() {
    if let mdast::Node::TableRow(row_node) = row {
      for (col_index, cell) in row_node.children.iter().enumerate() {
        if let mdast::Node::TableCell(cell_node) = cell {
          let cell_id = generate_id();
          let cell_block =
            create_table_cell_block(&cell_id, parent_id, row_index, col_index, &table.align);

          let paragraph_node = mdast::Node::Paragraph(mdast::Paragraph {
            children: cell_node.children.clone(),
            position: None,
          });

          let paragraph_block_id = generate_id();
          let paragraph_block = create_block(
            &paragraph_block_id,
            &paragraph_node,
            Some(cell_id.clone()),
            None,
          );

          document_data
            .blocks
            .insert(paragraph_block.id.clone(), paragraph_block);
          document_data.blocks.insert(cell_id.clone(), cell_block);
          update_children_map(document_data, Some(parent_id.to_string()), &cell_id);
          update_children_map(
            document_data,
            Some(cell_id.to_string()),
            &paragraph_block_id,
          );

          process_children(
            document_data,
            &cell_node.children,
            Some(paragraph_block_id.clone()),
            None,
          );
        }
      }
    }
  }
}

fn create_table_cell_block(
  id: &str,
  parent_id: &str,
  row: usize,
  col: usize,
  alignments: &[AlignKind],
) -> Block {
  let mut cell_data = HashMap::new();
  cell_data.insert("rowPosition".to_string(), row.into());
  cell_data.insert("colPosition".to_string(), col.into());

  if let Some(align) = alignments.get(col) {
    let align_str = match align {
      AlignKind::Left => "left",
      AlignKind::Right => "right",
      AlignKind::Center => "center",
      _ => "left",
    };
    cell_data.insert("align".to_string(), Value::String(align_str.to_string()));
  }

  Block {
    id: id.to_string(),
    ty: "table/cell".to_string(),
    data: cell_data,
    parent: parent_id.to_string(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some("text".to_string()),
  }
}
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Delta {
  ops: Vec<Operation>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct Operation {
  insert: String,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  attributes: Vec<(String, Value)>,
}

impl From<Value> for Operation {
  fn from(s: Value) -> Self {
    let s = s.as_object().unwrap();
    let insert = s.get("insert").unwrap().as_str().unwrap().to_string();
    let attributes = s
      .get("attributes")
      .map(|v| {
        v.as_object()
          .unwrap()
          .iter()
          .map(|(k, v)| (k.clone(), v.clone()))
          .collect()
      })
      .unwrap_or_default();
    Self { insert, attributes }
  }
}
impl From<Operation> for Value {
  fn from(op: Operation) -> Self {
    let attributes = op
      .attributes
      .iter()
      .map(|(k, v)| (k.clone(), v.clone()))
      .collect::<HashMap<String, Value>>();

    match attributes.is_empty() {
      true => json!({ "insert": op.insert }),
      false => json!({ "insert": op.insert, "attributes": attributes }),
    }
  }
}

impl Default for Delta {
  fn default() -> Self {
    Self::new()
  }
}

impl Delta {
  pub fn new() -> Self {
    Self { ops: Vec::new() }
  }

  pub fn insert(&mut self, value: String, attributes: Vec<(String, Value)>) {
    self.ops.push(Operation {
      insert: value,
      attributes,
    });
  }

  pub fn extend(&mut self, other: Delta) {
    self.ops.extend(other.ops);
  }

  pub fn to_json(&self) -> String {
    let ops: Vec<Value> = self
      .ops
      .iter()
      .map(|op| Value::from(op.to_owned()))
      .collect();

    serde_json::to_string(&ops).unwrap()
  }
}

fn process_inline_node(node: &mdast::Node, mut attributes: Vec<(String, Value)>) -> Delta {
  match node {
    mdast::Node::Text(text) => {
      let mut delta = Delta::new();
      delta.insert(text.value.clone(), attributes);
      delta
    },
    mdast::Node::Strong(strong) => {
      attributes.push(("bold".to_string(), Value::Bool(true)));
      process_children_inline(&strong.children, attributes)
    },
    mdast::Node::Emphasis(emph) => {
      attributes.push(("italic".to_string(), Value::Bool(true)));
      process_children_inline(&emph.children, attributes)
    },
    mdast::Node::Link(link) => {
      attributes.push(("href".to_string(), Value::String(link.url.clone())));
      process_children_inline(&link.children, attributes)
    },
    mdast::Node::InlineCode(code) => {
      attributes.push(("code".to_string(), Value::Bool(true)));

      let mut delta = Delta::new();
      delta.insert(code.value.clone(), attributes);
      delta
    },
    mdast::Node::InlineMath(math) => {
      attributes.push(("formula".to_string(), Value::String(math.value.clone())));

      let mut delta = Delta::new();
      delta.insert("$".to_string(), attributes);
      delta
    },
    mdast::Node::Delete(del) => {
      attributes.push(("strikethrough".to_string(), Value::Bool(true)));
      process_children_inline(&del.children, attributes)
    },
    _ => Delta::new(),
  }
}

fn process_children_inline(children: &[mdast::Node], attributes: Vec<(String, Value)>) -> Delta {
  let mut delta = Delta::new();
  for child in children {
    delta.extend(process_inline_node(child, attributes.clone()));
  }
  delta
}

fn insert_delta_to_text_map(document_data: &mut DocumentData, parent_id: &str, new_delta: Delta) {
  let text_map = document_data.meta.text_map.as_mut().unwrap();

  let mut existing_delta = text_map
    .get(parent_id)
    .map(|s| {
      let ops = serde_json::from_str::<Value>(s)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| Operation::from(v.clone()))
        .collect();
      Delta { ops }
    })
    .unwrap_or_default();
  existing_delta.extend(new_delta);
  text_map.insert(parent_id.to_string(), existing_delta.to_json());
}

pub fn insert_text_to_delta(
  delta_str: Option<String>,
  text: String,
  attributes: HashMap<String, Value>,
) -> String {
  let mut delta: Delta = serde_json::from_str::<Delta>(delta_str.as_deref().unwrap_or("[]"))
    .unwrap_or_else(|_| Delta::new());
  delta.insert(text, attributes.into_iter().collect());
  delta.to_json()
}

fn process_children(
  document_data: &mut DocumentData,
  children: &[mdast::Node],
  parent_id: Option<String>,
  list_type: Option<&str>,
) {
  for child in children {
    process_node(document_data, child, parent_id.clone(), None, list_type);
  }
}
