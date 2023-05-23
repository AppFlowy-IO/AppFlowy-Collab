use aws_config::environment::EnvironmentVariableCredentialsProvider;
use aws_credential_types::provider::ProvideCredentials;
pub use dynamodb::*;
pub use plugin::*;
use rusoto_credential::{ProfileProvider, ProvideAwsCredentials};

mod dynamodb;
mod plugin;

pub async fn is_enable_aws_dynamodb() -> bool {
  let credentials_provider = EnvironmentVariableCredentialsProvider::new();
  let result = credentials_provider.provide_credentials().await;
  if result.is_err() {
    if let Ok(profile_provider) = ProfileProvider::new() {
      return profile_provider.credentials().await.is_ok();
    }
  }
  result.is_ok()
}
