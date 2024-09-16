use crate::blocks::{Block, DocumentData, DocumentMeta};
use crate::document_data::generate_id;
use crate::error::DocumentError;
use crate::importer::define::*;
use markdown::mdast::AlignKind;
use markdown::{mdast, to_mdast, Constructs, ParseOptions};
use serde_json::Value;
use std::collections::HashMap;

use super::delta::Delta;
use super::util::*;

#[derive(Default)]
pub struct MDImporter {
  /// The parse options for the markdown parser.
  ///
  /// If not set, the default options will be used.
  /// The default parse options contain
  /// - Github Flavored Markdown (GFM) features.
  /// - math text, math flow, autolink features.
  /// - default Markdown features.
  pub parse_options: ParseOptions,
}

impl MDImporter {
  pub fn new(parse_options: Option<ParseOptions>) -> Self {
    let parse_options = parse_options.unwrap_or_else(|| ParseOptions {
      gfm_strikethrough_single_tilde: true,
      constructs: Constructs {
        math_text: true,
        math_flow: true,
        autolink: true,
        ..Constructs::gfm()
      },
      ..ParseOptions::gfm()
    });

    Self { parse_options }
  }

  pub fn import(&self, document_id: &str, md: &str) -> Result<DocumentData, DocumentError> {
    let md_node =
      to_mdast(md, &self.parse_options).map_err(|_| DocumentError::ParseMarkdownError)?;

    let mut document_data = DocumentData {
      page_id: document_id.to_string(),
      blocks: HashMap::new(),
      meta: DocumentMeta {
        children_map: HashMap::new(),
        text_map: Some(HashMap::new()),
      },
    };

    process_mdast_node(
      &mut document_data,
      &md_node,
      None,
      Some(document_id.to_string()),
      None,
    );

    Ok(document_data)
  }
}

/// This function will recursively process the mdast node and convert it to document blocks
/// The document blocks will be stored in the document data
fn process_mdast_node(
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
      if let Some(mdast::Node::Paragraph(para)) =
        get_mdast_node_children(node).and_then(|c| c.first())
      {
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

fn create_block(
  id: &str,
  node: &mdast::Node,
  parent_id: Option<String>,
  list_type: Option<&str>,
) -> Block {
  Block {
    id: id.to_string(),
    ty: mdast_node_type_to_block_type(node, list_type),
    data: mdast_node_to_block_data(node),
    parent: parent_id.unwrap_or_default(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some(TEXT_TYPE.to_string()),
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

fn process_table(document_data: &mut DocumentData, table: &mdast::Table, parent_id: &str) {
  for (row_index, row) in table.children.iter().enumerate() {
    if let mdast::Node::TableRow(row_node) = row {
      process_table_row(document_data, row_node, row_index, parent_id, &table.align);
    }
  }
}

fn process_table_row(
  document_data: &mut DocumentData,
  row_node: &mdast::TableRow,
  row_index: usize,
  parent_id: &str,
  align: &[AlignKind],
) {
  for (col_index, cell) in row_node.children.iter().enumerate() {
    if let mdast::Node::TableCell(cell_node) = cell {
      let cell_id = generate_id();
      let cell_block = create_table_cell_block(&cell_id, parent_id, row_index, col_index, align);
      document_data.blocks.insert(cell_id.clone(), cell_block);
      update_children_map(document_data, Some(parent_id.to_string()), &cell_id);

      let paragraph_block_id = create_paragraph_block(document_data, &cell_id);

      process_children(
        document_data,
        &cell_node.children,
        Some(paragraph_block_id.clone()),
        None,
      );
    }
  }
}

fn create_paragraph_block(document_data: &mut DocumentData, parent_id: &str) -> String {
  let paragraph_node = mdast::Node::Paragraph(mdast::Paragraph {
    children: Vec::new(),
    position: None,
  });

  let paragraph_block_id = generate_id();
  let paragraph_block = create_block(
    &paragraph_block_id,
    &paragraph_node,
    Some(parent_id.to_string()),
    None,
  );

  document_data
    .blocks
    .insert(paragraph_block_id.clone(), paragraph_block);
  update_children_map(
    document_data,
    Some(parent_id.to_string()),
    &paragraph_block_id,
  );

  paragraph_block_id
}

fn create_table_cell_block(
  id: &str,
  parent_id: &str,
  row: usize,
  col: usize,
  alignments: &[AlignKind],
) -> Block {
  let mut cell_data = HashMap::new();
  cell_data.insert(ROW_POSITION_FIELD.to_string(), row.into());
  cell_data.insert(COL_POSITION_FIELD.to_string(), col.into());

  if let Some(align) = alignments.get(col) {
    let align_str = match align {
      AlignKind::Left => ALIGN_LEFT,
      AlignKind::Right => ALIGN_RIGHT,
      AlignKind::Center => ALIGN_CENTER,
      _ => ALIGN_LEFT,
    };
    cell_data.insert(
      ALIGN_FIELD.to_string(),
      Value::String(align_str.to_string()),
    );
  }

  Block {
    id: id.to_string(),
    ty: TABLE_CELL_TYPE.to_string(),
    data: cell_data,
    parent: parent_id.to_string(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some(TEXT_TYPE.to_string()),
  }
}

fn process_children(
  document_data: &mut DocumentData,
  children: &[mdast::Node],
  parent_id: Option<String>,
  list_type: Option<&str>,
) {
  for child in children {
    process_mdast_node(document_data, child, parent_id.clone(), None, list_type);
  }
}
