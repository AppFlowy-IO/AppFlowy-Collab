use crate::blocks::{Block, DocumentData, DocumentMeta};
use crate::document_data::generate_id;
use crate::error::DocumentError;
use crate::importer::define::*;
use crate::importer::delta::Delta;
use crate::importer::util::*;
use markdown::mdast::AlignKind;
use markdown::{mdast, to_mdast, Constructs, ParseOptions};
use serde_json::Value;
use std::collections::HashMap;
use tracing::trace;

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

  pub fn import(&self, document_id: &str, md: String) -> Result<DocumentData, DocumentError> {
    let md_node =
      to_mdast(&md, &self.parse_options).map_err(|_| DocumentError::ParseMarkdownError)?;

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
  start_number: Option<u32>,
) {
  // If the node is an inline node, process it as an inline node
  if is_inline_node(node) {
    trace!("Processing inline node: {:?}", node);
    process_inline_mdast_node(document_data, node, parent_id);
    return;
  }

  trace!("Processing node: {:?}", node);
  // If the node is a list node, process it as a list node
  if let Some((children, list_type, start_number)) = get_mdast_node_info(node) {
    process_mdast_node_children(
      document_data,
      parent_id,
      children,
      Some(&list_type),
      start_number,
    );
    return;
  }

  // Process other nodes as normal nodes
  let id = block_id.unwrap_or_else(generate_id);

  let block = create_block(&id, node, parent_id.clone(), list_type, start_number);

  document_data.blocks.insert(id.clone(), block);

  update_children_map(document_data, parent_id, &id);

  match node {
    mdast::Node::Root(root) => {
      process_mdast_node_children(
        document_data,
        Some(id.clone()),
        &root.children,
        None,
        start_number,
      );
    },
    mdast::Node::Paragraph(para) => {
      process_mdast_node_children(
        document_data,
        Some(id.clone()),
        &para.children,
        None,
        start_number,
      );
    },
    mdast::Node::Heading(heading) => {
      process_mdast_node_children(
        document_data,
        Some(id.clone()),
        &heading.children,
        None,
        start_number,
      );
    },
    // handle the blockquote and list item node
    mdast::Node::BlockQuote(_) | mdast::Node::ListItem(_) => {
      if let Some(children) = get_mdast_node_children(node) {
        if children.is_empty() {
          return;
        }

        if let Some((first, rest)) = children.split_first() {
          // use the first node as the content of the block
          if let mdast::Node::Paragraph(para) = first {
            process_mdast_node_children(
              document_data,
              Some(id.clone()),
              &para.children,
              None,
              start_number,
            );
          }

          // continue to process the rest of the nodes
          process_mdast_node_children(
            document_data,
            Some(id.clone()),
            rest,
            list_type,
            start_number,
          );
        }
      }
    },
    mdast::Node::Code(code) => {
      let mut delta = Delta::new();
      delta.insert(code.value.clone(), Vec::new());
      insert_delta_to_text_map(document_data, &id, delta);
    },
    mdast::Node::Table(table) => process_table(document_data, table, &id),
    mdast::Node::Image(image) => process_image(document_data, image, &id),
    _ => {
      trace!("Unhandled node: {:?}", node);
    },
  }
}

fn create_block(
  id: &str,
  node: &mdast::Node,
  parent_id: Option<String>,
  list_type: Option<&str>,
  start_number: Option<u32>,
) -> Block {
  Block {
    id: id.to_string(),
    ty: mdast_node_type_to_block_type(node, list_type),
    data: mdast_node_to_block_data(node, start_number),
    parent: parent_id.unwrap_or_default(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some(BlockType::Text.to_string()),
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

fn process_image(document_data: &mut DocumentData, image: &mdast::Image, parent_id: &str) {
  let new_block_id = generate_id();
  let image_block = create_image_block(&new_block_id, image.url.clone(), parent_id);
  document_data
    .blocks
    .insert(new_block_id.clone(), image_block);
  update_children_map(document_data, Some(parent_id.to_string()), &new_block_id);
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

      process_mdast_node_children(
        document_data,
        Some(paragraph_block_id.clone()),
        &cell_node.children,
        None,
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

fn create_image_block(block_id: &str, url: String, parent_id: &str) -> Block {
  let mut data = BlockData::new();
  data.insert(URL_FIELD.to_string(), url.into());
  data.insert(IMAGE_TYPE_FIELD.to_string(), EXTERNAL_IMAGE_TYPE.into());
  Block {
    id: block_id.to_string(),
    ty: BlockType::Image.to_string(),
    data,
    parent: parent_id.to_string(),
    children: "".to_string(),
    external_id: None,
    external_type: None,
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
    ty: BlockType::TableCell.to_string(),
    data: cell_data,
    parent: parent_id.to_string(),
    children: id.to_string(),
    external_id: Some(id.to_string()),
    external_type: Some(BlockType::Text.to_string()),
  }
}

fn process_mdast_node_children(
  document_data: &mut DocumentData,
  parent_id: Option<String>,
  children: &[mdast::Node],
  list_type: Option<&str>,
  start_number: Option<u32>,
) {
  for child in children {
    process_mdast_node(
      document_data,
      child,
      parent_id.clone(),
      None,
      list_type,
      start_number,
    );
  }
}
