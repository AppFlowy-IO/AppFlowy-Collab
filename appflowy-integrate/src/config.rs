use std::str::FromStr;

use collab_plugins::cloud_storage::postgres::SupabaseDBConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CollabPluginConfig {
  /// Only one of the following two fields should be set.
  aws_config: Option<AWSDynamoDBConfig>,
  supabase_config: Option<SupabaseDBConfig>,
}

impl CollabPluginConfig {
  pub fn aws_config(&self) -> Option<&AWSDynamoDBConfig> {
    self.aws_config.as_ref()
  }

  pub fn set_aws_config(&mut self, aws_config: AWSDynamoDBConfig) -> Result<(), anyhow::Error> {
    if self.supabase_config.is_some() {
      return Err(anyhow::anyhow!(
        "Only one of the following two fields should be set: aws_config, supabase_config"
      ));
    }
    self.aws_config = Some(aws_config);
    Ok(())
  }

  pub fn supabase_config(&self) -> Option<&SupabaseDBConfig> {
    self.supabase_config.as_ref()
  }

  pub fn set_supabase_config(
    &mut self,
    supabase_config: SupabaseDBConfig,
  ) -> Result<(), anyhow::Error> {
    if self.aws_config.is_some() {
      return Err(anyhow::anyhow!(
        "Only one of the following two fields should be set: aws_config, supabase_config"
      ));
    }
    self.supabase_config = Some(supabase_config);
    Ok(())
  }
}

impl CollabPluginConfig {}

impl FromStr for CollabPluginConfig {
  type Err = serde_json::Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    serde_json::from_str(s)
  }
}

pub const AWS_ACCESS_KEY_ID: &str = "AWS_ACCESS_KEY_ID";
pub const AWS_SECRET_ACCESS_KEY: &str = "AWS_SECRET_ACCESS_KEY";
pub const AWS_REGION: &str = "AWS_REGION";

// To enable this test, you should set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY in your environment variables.
// or create the ~/.aws/credentials file following the instructions in https://docs.aws.amazon.com/sdk-for-rust/latest/dg/credentials.html
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct AWSDynamoDBConfig {
  pub access_key_id: String,
  pub secret_access_key: String,
  // Region list: https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/Concepts.RegionsAndAvailabilityZones.html
  pub region: String,
  pub enable: bool,
}

impl AWSDynamoDBConfig {
  pub fn from_env() -> Option<Self> {
    let access_key_id = std::env::var(AWS_ACCESS_KEY_ID).ok()?;
    let secret_access_key = std::env::var(AWS_SECRET_ACCESS_KEY).ok()?;
    let region = std::env::var(AWS_REGION).unwrap_or_else(|_| "us-east-1".to_string());
    Some(Self {
      access_key_id,
      secret_access_key,
      region,
      enable: true,
    })
  }

  pub fn write_env(&self) {
    std::env::set_var(AWS_ACCESS_KEY_ID, &self.access_key_id);
    std::env::set_var(AWS_SECRET_ACCESS_KEY, &self.secret_access_key);
    std::env::set_var(AWS_REGION, &self.region);
  }
}
