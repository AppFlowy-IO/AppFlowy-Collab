pub use plugin::*;

mod postgres_db;
pub use postgres_db::SupabaseDBConfig;
pub use postgres_db::UpdateTableConfig;
mod plugin;
