use aws_config::environment::EnvironmentVariableCredentialsProvider;
use aws_credential_types::provider::ProvideCredentials;
use rand::Rng;
use rusoto_credential::{ProfileProvider, ProvideAwsCredentials};

// To enable this test, you should set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY in your environment variables.
// or create the ~/.aws/credentials file following the instructions in https://docs.aws.amazon.com/sdk-for-rust/latest/dg/credentials.html
pub async fn is_enable_aws_test() -> bool {
  let credentials_provider = EnvironmentVariableCredentialsProvider::new();
  let result = credentials_provider.provide_credentials().await;
  if result.is_err() {
    if let Ok(profile_provider) = ProfileProvider::new() {
      return profile_provider.credentials().await.is_ok();
    }
  }
  result.is_ok()
}

pub fn generate_random_string(length: usize) -> String {
  const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
  let mut rng = rand::thread_rng();
  let random_string: String = (0..length)
    .map(|_| {
      let index = rng.gen_range(0..CHARSET.len());
      CHARSET[index] as char
    })
    .collect();

  random_string
}
