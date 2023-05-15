use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppFlowyCollabConfig {
  pub aws_config: Option<AWSDynamoDBConfig>,
}

impl Default for AppFlowyCollabConfig {
  fn default() -> Self {
    Self {
      aws_config: Some(AWSDynamoDBConfig {
        access_key_id: "AKIAYLLYLXWA2LS3BV6O".to_string(),
        secret_access_key: "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".to_string(),
      }),
    }
  }
}

impl AppFlowyCollabConfig {
  pub fn aws_config(&self) -> Option<&AWSDynamoDBConfig> {
    self.aws_config.as_ref()
  }
}

impl FromStr for AppFlowyCollabConfig {
  type Err = serde_json::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    serde_json::from_str(s)
  }
}

// To enable this test, you should set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY in your environment variables.
// or create the ~/.aws/credentials file following the instructions in https://docs.aws.amazon.com/sdk-for-rust/latest/dg/credentials.html
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct AWSDynamoDBConfig {
  pub access_key_id: String,
  pub secret_access_key: String,
}
