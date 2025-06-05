use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AttrKey {
  Bold,
  Italic,
  Strikethrough,
  Href,
  Code,
}

impl AttrKey {
  pub fn as_str(&self) -> &'static str {
    match self {
      AttrKey::Bold => "bold",
      AttrKey::Italic => "italic",
      AttrKey::Strikethrough => "strikethrough",
      AttrKey::Href => "href",
      AttrKey::Code => "code",
    }
  }
}

impl FromStr for AttrKey {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "bold" => Ok(AttrKey::Bold),
      "italic" => Ok(AttrKey::Italic),
      "strikethrough" => Ok(AttrKey::Strikethrough),
      "href" => Ok(AttrKey::Href),
      "code" => Ok(AttrKey::Code),
      _ => Err(format!("Unknown attribute key: {}", s)),
    }
  }
}
