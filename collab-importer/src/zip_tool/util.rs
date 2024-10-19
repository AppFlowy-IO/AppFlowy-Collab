use fancy_regex::Regex;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Check if the first 4 bytes of the buffer match known multi-part zip signatures.
pub fn is_multi_part_zip_signature(buffer: &[u8; 4]) -> bool {
  const MULTI_PART_SIGNATURES: [[u8; 4]; 2] = [
    [0x50, 0x4b, 0x07, 0x08], // Spanned zip signature
    [0x50, 0x4b, 0x03, 0x04], // Regular zip signature
  ];

  MULTI_PART_SIGNATURES.contains(buffer)
}

/// Async function to check if a file is a multi-part zip by reading the first 4 bytes.
pub async fn is_multi_part_zip(file_path: &Path) -> Result<bool, anyhow::Error> {
  let mut file = File::open(file_path).await?;
  let mut buffer = [0; 4]; // Read only the first 4 bytes
  file.read_exact(&mut buffer).await?;
  Ok(is_multi_part_zip_signature(&buffer))
}

/// Check if a buffer contains the multi-part zip signature.
pub fn is_multi_part_zip_file(buffer: &[u8; 4]) -> bool {
  is_multi_part_zip_signature(buffer)
}

pub fn sanitize_file_path(path: &str) -> PathBuf {
  // Replaces backwards slashes
  path.replace('\\', "/")
        // Sanitizes each component
        .split('/')
        .map(sanitize_filename::sanitize)
        .collect()
}

pub fn remove_part_suffix(file_name: &str) -> String {
  let path = Path::new(file_name);
  if let Some(stem) = path.file_stem() {
    let mut stem_str = stem.to_string_lossy().to_string();
    // Common patterns for multi-part files
    // Common patterns for multi-part files
    let patterns = [
      r"(?i)-part-\d+", // -Part-1, -Part-2, etc., case-insensitive
      r"(?i)\.z\d{2}",  // .z01, .z02, etc., case-insensitive
      r"(?i)\.part\d+", // .part1, .part2, etc., case-insensitive
      r"\(\d+\)",       // (1), (2), etc.
      r"_\d+",          // _1, _2, etc.
    ];

    for pattern in &patterns {
      let re = Regex::new(pattern).unwrap();
      stem_str = re.replace(&stem_str, "").to_string();
    }
    return stem_str;
  }

  file_name.to_string()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_remove_part_suffix() {
    let cases = vec![
      // Test cases with expected outputs
      (
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12-Part-1.zip",
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12",
      ),
      (
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12-part-1.zip",
        "Export-99d4faad-5bc1-4fef-82ac-5f6c7a957b12",
      ),
      ("file.z01", "file"),
      ("file.part2.zip", "file"),
      ("file(1).zip", "file"),
      ("file_3.zip", "file"),
      ("document-Part-10.zip", "document"),
      ("project.part1.zip", "project"),
      ("archive.z99.zip", "archive"),
      // Test case with no suffix
      ("normalfile.zip", "normalfile"),
      // Test case with no extension
      ("file-no-ext", "file-no-ext"),
    ];

    for (input, expected) in cases {
      assert_eq!(
        remove_part_suffix(input),
        expected,
        "Failed for input: {}",
        input
      );
    }
  }
}
