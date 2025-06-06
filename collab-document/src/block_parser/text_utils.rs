use crate::block_parser::OutputFormat;
use crate::block_parser::traits::ParseContext;
use crate::blocks::{AttrKey, Block, TextDelta};
use crate::error::DocumentError;
use collab::preclude::{Any, Attrs};

pub trait DocumentTextExtractor {
  /// Get the plain text or markdown text from the block
  fn extract_text_from_block(
    &self,
    block: &Block,
    context: &ParseContext,
  ) -> Result<String, DocumentError>;

  /// Get the plain text from the delta json string
  fn extract_plain_text_from_delta(&self, delta_json: &str) -> Result<String, DocumentError>;

  /// Get the markdown text from the delta json string
  fn extract_markdown_text_from_delta(&self, delta_json: &str) -> Result<String, DocumentError>;
}

pub struct DefaultDocumentTextExtractor;

impl DocumentTextExtractor for DefaultDocumentTextExtractor {
  fn extract_text_from_block(
    &self,
    block: &Block,
    context: &ParseContext,
  ) -> Result<String, DocumentError> {
    let external_id = block.external_id.as_ref();
    if let Some(external_id) = external_id {
      let delta_json = context
        .document_data
        .meta
        .text_map
        .as_ref()
        .and_then(|text_map| text_map.get(external_id));

      return match delta_json {
        Some(json) => match context.format {
          OutputFormat::PlainText => self.extract_plain_text_from_delta(json),
          OutputFormat::Markdown => self.extract_markdown_text_from_delta(json),
        },
        None => Ok(String::new()),
      };
    }

    Ok(String::new())
  }

  fn extract_plain_text_from_delta(&self, delta_json: &str) -> Result<String, DocumentError> {
    let deltas: Vec<TextDelta> = serde_json::from_str(delta_json)
      .map_err(|_| DocumentError::ParseDeltaJsonToTextDeltaError)?;
    let result = deltas
      .into_iter()
      .filter_map(|delta| {
        if let TextDelta::Inserted(text, attributes) = delta {
          // TODO: mention logic should be delegated to the parser
          if text == "$" {
            if let Some(attrs) = attributes {
              if let Some(Any::String(_)) = attrs.get(AttrKey::Mention.as_str()) {
                return Some("".to_string());
              }
            }
          }
          Some(text)
        } else {
          None
        }
      })
      .collect::<String>();

    Ok(result)
  }

  fn extract_markdown_text_from_delta(&self, delta_json: &str) -> Result<String, DocumentError> {
    let deltas: Vec<TextDelta> = serde_json::from_str(delta_json)
      .map_err(|_| DocumentError::ParseDeltaJsonToTextDeltaError)?;
    let result = deltas
      .into_iter()
      .filter_map(|delta| {
        if let TextDelta::Inserted(text, attributes) = delta {
          Some(match attributes {
            Some(attrs) => format_text_with_attributes(&text, &attrs),
            None => text,
          })
        } else {
          None
        }
      })
      .collect::<String>();

    Ok(result)
  }
}

pub fn format_text_with_attributes(text: &str, attributes: &Attrs) -> String {
  let mut result = text.to_string();

  if let Some(Any::Bool(true)) = attributes.get(AttrKey::Bold.as_str()) {
    result = format!("**{}**", result);
  }

  if let Some(Any::Bool(true)) = attributes.get(AttrKey::Italic.as_str()) {
    result = format!("*{}*", result);
  }

  if let Some(Any::Bool(true)) = attributes.get(AttrKey::Strikethrough.as_str()) {
    result = format!("~~{}~~", result);
  }

  if let Some(href) = attributes.get(AttrKey::Href.as_str()) {
    let href_str = href.to_string();
    result = format!("[{}]({})", result, href_str);
  }

  if let Some(Any::Bool(true)) = attributes.get(AttrKey::Code.as_str()) {
    result = format!("`{}`", result);
  }

  result
}
