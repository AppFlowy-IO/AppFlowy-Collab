use crate::notion::page::ImportedRowDocument;
use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use serde::Serialize;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub enum LinkType {
  Unknown,
  CSV,
  Markdown,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize)]
pub enum NotionFile {
  #[default]
  Empty,
  CSV {
    file_path: PathBuf,
    size: u64,
    resources: Vec<Resource>,
    row_documents: Vec<ImportedRowDocument>,
  },
  CSVPart {
    file_path: PathBuf,
    size: u64,
  },
  Markdown {
    file_path: PathBuf,
    size: u64,
    resources: Vec<Resource>,
  },
}

impl NotionFile {
  pub fn is_markdown(&self) -> bool {
    matches!(self, NotionFile::Markdown { .. })
  }

  pub fn is_csv_all(&self) -> bool {
    matches!(self, NotionFile::CSV { .. })
  }
  pub fn imported_file_path(&self) -> Option<&PathBuf> {
    match self {
      NotionFile::CSV { file_path, .. } => Some(file_path),
      NotionFile::Markdown { file_path, .. } => Some(file_path),
      _ => None,
    }
  }
  pub fn upload_files(&self) -> Vec<PathBuf> {
    match self {
      NotionFile::Markdown { resources, .. } => resources
        .iter()
        .flat_map(|r| r.file_paths())
        .cloned()
        .collect(),
      NotionFile::CSV { resources, .. } => resources
        .iter()
        .flat_map(|r| r.file_paths())
        .cloned()
        .collect(),
      _ => vec![],
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum Resource {
  Images { files: Vec<(PathBuf, u64)> },
  Files { files: Vec<(PathBuf, u64)> },
}

impl Resource {
  pub fn file_paths(&self) -> Vec<&PathBuf> {
    match self {
      Resource::Images { files } => files.iter().map(|(path, _)| path).collect(),
      Resource::Files { files } => files.iter().map(|(path, _)| path).collect(),
    }
  }
  pub fn size(&self) -> u64 {
    match self {
      Resource::Images { files } => files.iter().map(|(_, size)| *size).sum(),
      Resource::Files { files } => files.iter().map(|(_, size)| *size).sum(),
    }
  }
  pub fn contains(&self, path: &PathBuf) -> bool {
    match self {
      Resource::Images { files } => files.iter().any(|(file_path, _)| file_path == path),
      Resource::Files { files } => files.iter().any(|(file_path, _)| file_path == path),
    }
  }
}

fn remove_first_h1_until_next_h2(md_content: &str) -> String {
  // Parse the Markdown content into an AST
  let parse_options = ParseOptions::default();
  let mut ast = to_mdast(md_content, &parse_options).unwrap();

  if let Node::Root(root) = &mut ast {
    let mut inside_h1_block = false;

    // Filter the root’s children to remove the first H1 and content until the next H2
    root.children.retain(|node| {
      if inside_h1_block {
        // If we're inside the H1 block, check if this node is an H2 heading
        if let Node::Heading(heading) = node {
          if heading.depth == 2 {
            inside_h1_block = false; // Stop removing content
            return true; // Keep the H2 heading
          }
        }
        // Skip the current node as it's part of the H1 block
        return false;
      }

      // Check if the current node is the first H1 heading
      if let Node::Heading(heading) = node {
        if heading.depth == 1 {
          inside_h1_block = true; // Start removing content
          return false; // Remove the H1 heading
        }
      }

      // Keep the node if it's not part of the H1 block
      true
    });
  }

  // Convert the modified AST back to Markdown
  ast.to_string()
}

pub fn process_row_md_file(file_path: &PathBuf) -> io::Result<()> {
  let md_content = fs::read_to_string(file_path)?;
  let updated_md = remove_first_h1_until_next_h2(&md_content);
  let mut file = File::create(file_path)?;
  file.write_all(updated_md.as_bytes())?;

  Ok(())
}
