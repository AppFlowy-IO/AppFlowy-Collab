use rusoto_core::Region;
use rusoto_dynamodb::{
  AttributeValue, DeleteItemInput, DynamoDb, DynamoDbClient, GetItemInput, ListTablesInput,
  PutItemInput,
};
use std::fmt::Debug;
use std::ops::RangeBounds;

pub struct DynamicDBPlugin {
  table_name: String,
  client: DynamoDbClient,
}

impl DynamicDBPlugin {
  pub async fn new(table_name: &str) -> Self {
    let table_name = table_name.to_string();
    let region = Region::default();
    let client = DynamoDbClient::new(region);

    let list_tables_input: ListTablesInput = Default::default();

    match client.list_tables(list_tables_input).await {
      Ok(output) => match output.table_names {
        Some(table_name_list) => {
          println!("Tables in database:");

          for table_name in table_name_list {
            println!("{}", table_name);
          }
        },
        None => println!("No tables in database!"),
      },
      Err(error) => {
        println!("Error: {:?}", error);
      },
    }
    DynamicDBPlugin { table_name, client }
  }
}
