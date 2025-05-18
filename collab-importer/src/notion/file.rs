use crate::notion::page::ImportedRowDocument;
use markdown::mdast::Node;
use markdown::{ParseOptions, to_mdast};
use serde::Serialize;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub enum LinkType {
  Unknown,
  CSV,
  Markdown,
}

#[derive(Debug, Default, Clone)]
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

  pub fn is_csv(&self) -> bool {
    matches!(self, NotionFile::CSV { .. })
  }
  pub fn file_path(&self) -> Option<&PathBuf> {
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

pub fn remove_text_between_first_h1_and_second_empty_line(md_content: &str) -> String {
  let parse_options = ParseOptions::default();
  let ast = to_mdast(md_content, &parse_options).unwrap();

  // Variables to track the line range to remove
  let mut start_line = None;
  let mut empty_line_count = 0;
  let mut end_line = None;

  // Traverse the AST to find the first H1
  if let Node::Root(root) = &ast {
    for node in &root.children {
      if let Node::Heading(heading) = node {
        if heading.depth == 1 && start_line.is_none() {
          // Mark the start line of the H1 block
          start_line = heading.position.as_ref().map(|pos| pos.start.line);
          break;
        }
      }
    }
  }

  if start_line.is_none() {
    return md_content.to_string();
  }

  // find the second empty line after the first H1
  let lines = md_content.lines().collect::<Vec<_>>();
  for (index, line) in lines.iter().enumerate() {
    if line.trim().is_empty() && index > start_line.unwrap() {
      empty_line_count += 1;
    }
    if empty_line_count == 2 {
      end_line = Some(index);
      break;
    }
  }

  let start_line = start_line.unwrap_or(0);
  // if the second empty line is not found, remove the text after the first H1
  let end_line = end_line.unwrap_or(lines.len() + 1);
  // Filter the lines and remove the lines between start_line and end_line
  let result: String = md_content
    .lines()
    .enumerate()
    .filter_map(|(index, line)| {
      let line_num = index + 1;
      if line_num < start_line || line_num >= end_line {
        Some(line)
      } else {
        None
      }
    })
    .collect::<Vec<_>>()
    .join("\n");

  result
}

pub fn process_row_md_content(md_content: String, file_path: &PathBuf) -> io::Result<()> {
  let updated_md = remove_text_between_first_h1_and_second_empty_line(&md_content);
  if updated_md.is_empty() {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "The Markdown content is empty after processing",
    ));
  }
  let mut file = File::create(file_path)?;
  file.write_all(updated_md.as_bytes())?;
  Ok(())
}
