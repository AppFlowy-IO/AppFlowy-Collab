use serde::{Deserialize, Serialize};

pub const SUPABASE_URL: &str = "SUPABASE_URL";
pub const SUPABASE_ANON_KEY: &str = "SUPABASE_ANON_KEY";
pub const SUPABASE_KEY: &str = "SUPABASE_KEY";
pub const SUPABASE_JWT_SECRET: &str = "SUPABASE_JWT_SECRET";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SupabaseDBConfig {
  /// The url of the supabase server.
  pub url: String,
  /// The key of the supabase server.
  pub key: String,
  /// The secret used to sign the JWT tokens.
  pub jwt_secret: String,
}

impl SupabaseDBConfig {
  pub fn from_env() -> Option<Self> {
    Some(Self {
      url: std::env::var(SUPABASE_URL).ok()?,
      key: std::env::var(SUPABASE_KEY).ok()?,
      jwt_secret: std::env::var(SUPABASE_JWT_SECRET).ok()?,
    })
  }

  pub fn write_env(&self) {
    tracing::trace!("write env: {:?}", self);
    std::env::set_var(SUPABASE_URL, &self.url);
    std::env::set_var(SUPABASE_KEY, &self.key);
    std::env::set_var(SUPABASE_JWT_SECRET, &self.jwt_secret);
  }
}
