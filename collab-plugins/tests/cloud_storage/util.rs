use std::env::var;

pub(crate) fn is_enable_aws_test() -> bool {
  let value = var("AWS_ACCESS_KEY_ID").is_ok() && var("AWS_SECRET_ACCESS_KEY").is_ok();
  if !value {}
  value
}

struct CredentialConfig {
  aws_access_key_id: String,
  aws_secret_access_key: String,
}
