#![allow(clippy::all)]

use anyhow::Error;
use tokio_postgres::Transaction;
use tracing::instrument;

pub struct Tables {
  pub af_user: String,
  pub af_user_profile: String,
  pub af_workspace: String,
  pub af_collab: String,
}

impl Default for Tables {
  fn default() -> Self {
    Self {
      af_user: "af_user".to_string(),
      af_user_profile: "af_user_profile".to_string(),
      af_workspace: "af_workspace".to_string(),
      af_collab: "af_collab".to_string(),
    }
  }
}
/// Create tables if not exists.
#[allow(dead_code)]
pub async fn create_tables(transaction: Transaction<'_>) -> Result<(), anyhow::Error> {
  let tables = Tables::default();
  create_af_user_table(&transaction, &tables.af_user).await?;
  create_af_user_profile_table(&transaction, &tables.af_user_profile, &tables.af_user).await?;
  create_af_workspace_table(&transaction, &tables.af_workspace, &tables.af_user_profile).await?;
  create_af_collab_table(&transaction, &tables.af_collab).await?;
  transaction.commit().await?;
  Ok(())
}

#[allow(dead_code)]
pub async fn reset(transaction: Transaction<'_>) -> Result<(), anyhow::Error> {
  let tables = Tables::default();
  drop_table(&transaction, &tables.af_collab).await?;
  drop_table(&transaction, &tables.af_workspace).await?;
  drop_table(&transaction, &tables.af_user_profile).await?;
  drop_table(&transaction, &tables.af_user).await?;

  drop_trigger(
    &transaction,
    &make_trigger_name(&tables.af_workspace),
    &tables.af_workspace,
  )
  .await?;
  drop_trigger_functions(&transaction, &make_trigger_func_name(&tables.af_workspace)).await?;

  drop_trigger(
    &transaction,
    &make_trigger_name(&tables.af_user_profile),
    &tables.af_user_profile,
  )
  .await?;
  drop_trigger_functions(
    &transaction,
    &make_trigger_func_name(&tables.af_user_profile),
  )
  .await?;

  transaction.commit().await?;
  Ok(())
}

#[instrument(level = "trace", err, skip(client))]
pub(crate) async fn create_af_user_table(
  client: &Transaction<'_>,
  name: &str,
) -> Result<(), anyhow::Error> {
  tracing::trace!("creating af_user table");
  let statement = &format!(
    "CREATE TABLE IF NOT EXISTS {} (
       uuid UUID PRIMARY KEY,
       uid BIGINT GENERATED ALWAYS AS IDENTITY,
       created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
  )",
    name
  );
  client.execute(statement, &[]).await?;
  Ok(())
}

pub(crate) async fn drop_table(client: &Transaction<'_>, table: &str) -> Result<(), Error> {
  let statement = &format!("DROP TABLE IF EXISTS {}", table);
  client.execute(statement, &[]).await?;
  Ok(())
}

pub(crate) async fn drop_trigger_functions(
  client: &Transaction<'_>,
  func: &str,
) -> Result<(), Error> {
  let statement = &format!("DROP FUNCTION IF EXISTS {}", func);
  client.execute(statement, &[]).await?;
  Ok(())
}

pub(crate) async fn drop_trigger(
  client: &Transaction<'_>,
  trigger: &str,
  table: &str,
) -> Result<(), Error> {
  let statement = &format!("DROP TRIGGER IF EXISTS {} ON {}", trigger, table);
  client.execute(statement, &[]).await?;
  Ok(())
}

fn make_trigger_func_name(table: &str) -> String {
  format!("create_{}_trigger_func", table)
}

fn make_trigger_name(table: &str) -> String {
  format!("create_{}_trigger", table)
}

/// When a new record of the af_user_table is created, a new record of the af_user_profile_table is created.
/// The af_user_profile_table is used to store the user's name, email, and workspace_id.
#[instrument(level = "trace", err, skip(client))]
pub(crate) async fn create_af_user_profile_table(
  client: &Transaction<'_>,
  user_profile_table: &str,
  user_table: &str,
) -> Result<(), anyhow::Error> {
  let statement = &format!(
    "CREATE TABLE IF NOT EXISTS {} (
       uid BIGINT PRIMARY KEY,
       uuid UUID,
       name TEXT,
       email TEXT,
       workspace_id UUID DEFAULT uuid_generate_v4()
  )",
    user_profile_table
  );
  client.execute(statement, &[]).await?;

  // Create the trigger function
  let trigger_name = make_trigger_func_name(user_profile_table);
  let statement = &format!(
    "CREATE OR REPLACE FUNCTION {}()
        RETURNS TRIGGER AS $$
        BEGIN
            INSERT INTO {} (uid,uuid) VALUES (NEW.uid, NEW.uuid);
            RETURN NEW;
        END
        $$ LANGUAGE plpgsql;
    ",
    trigger_name, user_profile_table
  );
  client.execute(statement, &[]).await?;

  // Create the trigger
  let statement = &format!(
    "CREATE TRIGGER {}
        AFTER INSERT ON {} 
        FOR EACH ROW
        EXECUTE FUNCTION {}();
    ",
    make_trigger_name(user_profile_table),
    user_table,
    trigger_name
  );

  client.execute(statement, &[]).await?;
  Ok(())
}

/// When a new record of the af_user_profile_table is created, a new record of the af_workspace_table
/// is created. The workspace_id will be the same as the one in the af_user_profile_table.
#[instrument(level = "trace", err, skip(client))]
pub(crate) async fn create_af_workspace_table(
  client: &Transaction<'_>,
  workspace_table: &str,
  user_profile_table: &str,
) -> Result<(), anyhow::Error> {
  let statement = &format!(
    "CREATE TABLE IF NOT EXISTS {} (
       workspace_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
       uid BIGINT,
       created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
       workspace_name TEXT DEFAULT 'My Workspace'
  )",
    workspace_table
  );
  client.execute(statement, &[]).await?;

  // Create the trigger function
  let trigger_name = make_trigger_func_name(workspace_table);
  let statement = &format!(
    "CREATE OR REPLACE FUNCTION {}()
        RETURNS TRIGGER AS $$
        BEGIN
            INSERT INTO {} (uid,workspace_id) VALUES (NEW.uid, NEW.workspace_id);
            RETURN NEW;
        END
        $$ LANGUAGE plpgsql;
    ",
    trigger_name, workspace_table
  );
  client.execute(statement, &[]).await?;

  // Create the trigger
  let statement = &format!(
    "CREATE TRIGGER {} 
        AFTER INSERT ON {} 
        FOR EACH ROW
        EXECUTE FUNCTION {}();
    ",
    make_trigger_name(workspace_table),
    user_profile_table,
    trigger_name
  );
  client.execute(statement, &[]).await?;
  Ok(())
}

#[instrument(level = "trace", err, skip(client))]
pub(crate) async fn create_af_collab_table(
  client: &Transaction<'_>,
  name: &str,
) -> Result<(), anyhow::Error> {
  let statement = &format!(
    "CREATE TABLE IF NOT EXISTS {} (
     oid TEXT,
     key BIGINT GENERATED ALWAYS AS IDENTITY,
     value TEXT NOT NULL,
     PRIMARY KEY (oid, key)
  )",
    name
  );

  match client.execute(statement, &[]).await {
    Ok(_) => {
      tracing::trace!("table {} created success", name);
      Ok(())
    },
    Err(e) => {
      tracing::error!("table {} creation error: {}", name, e);
      Err(anyhow::Error::from(e))
    },
  }
}

#[cfg(test)]
mod tests {
  use crate::cloud_storage::postgres::postgres_table::{PostgresConfiguration, PostgresDB};
  use crate::cloud_storage::postgres::sql::*;

  const ENV_FILE: &str = ".env.test.danger";

  async fn make_db() -> Option<PostgresDB> {
    dotenv::from_filename(ENV_FILE).ok()?;
    PostgresDB::from_env().await.ok()
  }

  // ‼️‼️‼️ Warning: this test will create a table in the database
  #[tokio::test]
  async fn remove_all_tables_test() {
    if dotenv::from_filename(ENV_FILE).is_err() {
      return;
    }

    let configuration = PostgresConfiguration::from_env()
      .ok_or(anyhow::anyhow!("PostgresConfiguration not found in env"))
      .unwrap();

    let mut config = tokio_postgres::Config::new();
    config
      .host(&configuration.url)
      .user(&configuration.user_name)
      .password(&configuration.password)
      .port(configuration.port);

    let (mut client, connection) = config.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move {
      if let Err(e) = connection.await {
        tracing::error!("postgres db connection error: {}", e);
      }
    });

    let txn = client.transaction().await.unwrap();
    reset(txn).await.unwrap();
  }

  // ‼️‼️‼️ Warning: this test will create a table in the database
  #[tokio::test]
  async fn create_collab_table() {
    if let Some(mut db) = make_db().await {
      let txn = db.client.transaction().await.unwrap();
      create_af_collab_table(&txn, "af_collab").await.unwrap();
    }
  }

  // ‼️‼️‼️ Warning: this test will create a table in the database
  #[tokio::test]
  async fn create_user_table() {
    if let Some(mut db) = make_db().await {
      let txn = db.client.transaction().await.unwrap();
      create_af_user_table(&txn, "af_user").await.unwrap();
    }
  }

  // ‼️‼️‼️Warning: this test will create a table in the database
  #[tokio::test]
  async fn create_user_profile_table() {
    if let Some(mut db) = make_db().await {
      let txn = db.client.transaction().await.unwrap();
      create_af_user_profile_table(&txn, "af_user_profile2", "af_user")
        .await
        .unwrap();
    }
  }

  // ‼️‼️‼️ Warning: this test will create a table in the database
  #[tokio::test]
  async fn create_workspace_table() {
    if let Some(mut db) = make_db().await {
      let txn = db.client.transaction().await.unwrap();
      create_af_workspace_table(&txn, "af_workspace2", "af_user_profile2")
        .await
        .unwrap();
    }
  }
}
