use super::delta::{Delta, Operation};
use crate::{
  blocks::{BlockType, DocumentData},
  importer::define::*,
};
use markdown::mdast;
use serde_json::Value;
use std::collections::HashMap;
use tracing::trace;

pub type BlockData = HashMap<String, Value>;

/// Convert the node type to string
pub(crate) fn mdast_node_type_to_block_type(node: &mdast::Node, list_type: Option<&str>) -> String {
  match node {
    mdast::Node::Root(_) => BlockType::Page,
    mdast::Node::Paragraph(_) => BlockType::Paragraph,
    mdast::Node::Heading(_) => BlockType::Heading,
    mdast::Node::Blockquote(_) => BlockType::Quote,
    mdast::Node::Code(_) => BlockType::Code,
    mdast::Node::Image(_) => BlockType::Image,
    mdast::Node::ImageReference(_) => BlockType::Image,
    mdast::Node::LinkReference(_) => BlockType::LinkPreview,
    mdast::Node::Math(_) => BlockType::MathEquation,
    mdast::Node::ThematicBreak(_) => BlockType::Divider,
    mdast::Node::Table(_) => BlockType::SimpleTable,
    mdast::Node::TableCell(_) => BlockType::SimpleTableCell,
    mdast::Node::ListItem(list) => {
      if list.checked.is_some() {
        BlockType::TodoList
      } else {
        match list_type {
          None => BlockType::BulletedList,
          Some(s) => {
            let ty = BlockType::from_block_ty(s);
            if matches!(ty, BlockType::Custom(_)) {
              BlockType::BulletedList
            } else {
              ty
            }
          },
        }
      }
    },
    mdast::Node::Definition(defi) => {
      if is_image_url(&defi.url) {
        BlockType::Image
      } else {
        BlockType::LinkPreview
      }
    },
    _ => {
      trace!(
        "Unknown node type: {:?}, fallback to BlockType::Paragraph",
        node
      );
      BlockType::Paragraph
    },
  }
  .to_string()
}

/// Convert the mdast node to block data
pub(crate) fn mdast_node_to_block_data(node: &mdast::Node, start_number: Option<u32>) -> BlockData {
  let mut data = BlockData::new();

  match node {
    mdast::Node::Heading(heading) => {
      let level = heading.depth.clamp(1, 6);
      data.insert(LEVEL_FIELD.to_string(), level.into());
    },
    mdast::Node::Code(code) => {
      let language = code.lang.as_ref().cloned().unwrap_or_default();
      data.insert(LANGUAGE_FIELD.to_string(), language.into());
    },
    mdast::Node::Image(image) => {
      data.insert(URL_FIELD.to_string(), image.url.clone().into());
      data.insert(IMAGE_TYPE_FIELD.to_string(), EXTERNAL_IMAGE_TYPE.into());
    },
    mdast::Node::ImageReference(image) => {
      data.insert(URL_FIELD.to_string(), image.identifier.clone().into());
      data.insert(IMAGE_TYPE_FIELD.to_string(), EXTERNAL_IMAGE_TYPE.into());
    },
    mdast::Node::LinkReference(link) => {
      data.insert(URL_FIELD.to_string(), link.identifier.clone().into());
    },
    mdast::Node::Math(math) => {
      data.insert(FORMULA_FIELD.to_string(), math.value.clone().into());
    },
    mdast::Node::Table(_table) => {
      // SimpleTable doesn't need special data fields
    },
    mdast::Node::ListItem(list) => {
      if let Some(checked) = list.checked {
        data.insert(CHECKED_FIELD.to_string(), checked.into());
      }

      if let Some(start_number) = start_number {
        data.insert(START_NUMBER_FIELD.to_string(), start_number.into());
      }
    },
    mdast::Node::Definition(defi) => {
      let url = defi.url.to_string();
      if is_image_url(&url) {
        data.insert(IMAGE_TYPE_FIELD.to_string(), EXTERNAL_IMAGE_TYPE.into());
      }
      data.insert(URL_FIELD.to_string(), url.into());
    },
    _ => {},
  }
  data
}

/// Check if the url is an image url
///
/// NOTES: This function can't handle the case if the url points to an image but not
/// ends with the image extensions.
pub(crate) fn is_image_url(url: &str) -> bool {
  IMAGE_EXTENSIONS
    .iter()
    .any(|ext| url.to_lowercase().ends_with(ext))
}

/// Check if the node is an inline node
pub(crate) fn is_inline_node(node: &mdast::Node) -> bool {
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

/// Get the list type and children of the md ast node
pub(crate) fn get_mdast_node_info(
  node: &mdast::Node,
) -> Option<(&Vec<mdast::Node>, String, Option<u32>)> {
  if let mdast::Node::List(list) = node {
    let list_type = if list.ordered {
      BlockType::NumberedList
    } else {
      BlockType::BulletedList
    };
    Some((&list.children, list_type.as_str().to_string(), list.start))
  } else {
    None
  }
}

pub(crate) fn get_mdast_node_children(node: &mdast::Node) -> Option<&Vec<mdast::Node>> {
  match node {
    mdast::Node::Blockquote(quote) => Some(&quote.children),
    mdast::Node::ListItem(list) => Some(&list.children),
    _ => None,
  }
}

/// Process the inline node
pub(crate) fn process_inline_mdast_node(
  document_data: &mut DocumentData,
  node: &mdast::Node,
  parent_id: Option<String>,
) {
  if let Some(parent_id) = parent_id {
    let delta = inline_mdast_node_to_delta(node, Vec::new());
    insert_delta_to_text_map(document_data, &parent_id, delta);
  }
}

pub(crate) fn inline_mdast_node_to_delta(
  node: &mdast::Node,
  mut attributes: Vec<(String, Value)>,
) -> Delta {
  match node {
    mdast::Node::Text(text) => {
      let mut delta = Delta::new();
      delta.insert(text.value.clone(), attributes);
      delta
    },
    mdast::Node::Strong(strong) => {
      attributes.push((BOLD_ATTR.to_owned(), Value::Bool(true)));
      process_children_inline(&strong.children, attributes)
    },
    mdast::Node::Emphasis(emph) => {
      attributes.push((ITALIC_ATTR.to_owned(), Value::Bool(true)));
      process_children_inline(&emph.children, attributes)
    },
    mdast::Node::Link(link) => {
      attributes.push((HREF_ATTR.to_owned(), Value::String(link.url.clone())));
      process_children_inline(&link.children, attributes)
    },
    mdast::Node::InlineCode(code) => {
      attributes.push((CODE_ATTR.to_owned(), Value::Bool(true)));
      let mut delta = Delta::new();
      delta.insert(code.value.clone(), attributes);
      delta
    },
    mdast::Node::InlineMath(math) => {
      attributes.push((FORMULA_ATTR.to_owned(), Value::String(math.value.clone())));
      let mut delta = Delta::new();
      delta.insert(INLINE_MATH_SYMBOL.to_owned(), attributes);
      delta
    },
    mdast::Node::Delete(del) => {
      attributes.push((STRIKETHROUGH_ATTR.to_owned(), Value::Bool(true)));
      process_children_inline(&del.children, attributes)
    },
    _ => Delta::new(),
  }
}

pub(crate) fn process_children_inline(
  children: &[mdast::Node],
  attributes: Vec<(String, Value)>,
) -> Delta {
  let mut delta = Delta::new();
  for child in children {
    delta.extend(inline_mdast_node_to_delta(child, attributes.clone()));
  }
  delta
}

pub(crate) fn insert_delta_to_text_map(
  document_data: &mut DocumentData,
  parent_id: &str,
  new_delta: Delta,
) {
  let text_map = match document_data.meta.text_map.as_mut() {
    Some(map) => map,
    None => {
      eprintln!("Text map not found");
      return;
    },
  };

  let existing_delta = if let Some(s) = text_map.get(parent_id) {
    match serde_json::from_str::<Value>(s) {
      Ok(value) => {
        let ops = value
          .as_array()
          .map(|arr| {
            arr
              .iter()
              .filter_map(|v| Operation::try_from(v.clone()).ok())
              .collect()
          })
          .unwrap_or_default();
        Delta { ops }
      },
      Err(e) => {
        eprintln!("Failed to parse JSON: {}", e);
        Delta::default()
      },
    }
  } else {
    Delta::default()
  };

  let mut combined_delta = existing_delta;
  combined_delta.extend(new_delta);

  let json_string = combined_delta.to_json();
  text_map.insert(parent_id.to_string(), json_string);
}

// pub(crate) fn insert_text_to_delta(
//   delta_str: Option<String>,
//   text: String,
//   attributes: HashMap<String, Value>,
// ) -> String {
//   let mut delta = delta_str
//     .and_then(|s| serde_json::from_str::<Delta>(&s).ok())
//     .unwrap_or_default();

//   delta.insert(text, attributes.into_iter().collect());
//   delta.to_json()
// }
