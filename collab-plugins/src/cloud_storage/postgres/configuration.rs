use serde::{Deserialize, Serialize};
pub const SUPABASE_URL: &str = "SUPABASE_URL";
pub const SUPABASE_ANON_KEY: &str = "SUPABASE_ANON_KEY";
pub const SUPABASE_KEY: &str = "SUPABASE_KEY";
pub const SUPABASE_JWT_SECRET: &str = "SUPABASE_JWT_SECRET";
pub const SUPABASE_UPDATE_TABLE_NAME: &str = "SUPABASE_UPDATE_TABLE_NAME";
pub const SUPABASE_UPDATE_TABLE_ENABLE: &str = "SUPABASE_UPDATE_TABLE_ENABLE";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SupabaseDBConfig {
  /// The url of the supabase server.
  pub url: String,
  /// The key of the supabase server.
  pub key: String,
  /// The secret used to sign the JWT tokens.
  pub jwt_secret: String,
  /// Store the [Collab] updates in the update table.
  pub update_table_config: UpdateTableConfig,
}

impl SupabaseDBConfig {
  pub fn from_env() -> Option<Self> {
    Some(Self {
      url: std::env::var(SUPABASE_URL).ok()?,
      key: std::env::var(SUPABASE_KEY).ok()?,
      jwt_secret: std::env::var(SUPABASE_JWT_SECRET).ok()?,
      update_table_config: UpdateTableConfig::from_env().ok()?,
    })
  }

  pub fn write_env(&self) {
    std::env::set_var(SUPABASE_URL, &self.url);
    std::env::set_var(SUPABASE_KEY, &self.key);
    std::env::set_var(SUPABASE_JWT_SECRET, &self.jwt_secret);
    self.update_table_config.write_env();
  }
}

/// UpdateTable is used to store the updates of the collab object.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateTableConfig {
  pub table_name: String,
  /// Whether to enable the update table.
  /// If it's disabled, the updates will be stored in the object table.
  pub enable: bool,
}

impl UpdateTableConfig {
  pub fn write_env(&self) {
    std::env::set_var(SUPABASE_UPDATE_TABLE_NAME, &self.table_name);
    std::env::set_var(SUPABASE_UPDATE_TABLE_ENABLE, &self.enable.to_string());
  }

  pub fn from_env() -> Result<Self, anyhow::Error> {
    Ok(Self {
      table_name: std::env::var(SUPABASE_UPDATE_TABLE_NAME)?,
      enable: std::env::var(SUPABASE_UPDATE_TABLE_ENABLE)?
        .parse::<bool>()
        .unwrap_or(false),
    })
  }
}
