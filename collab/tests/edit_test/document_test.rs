#![allow(clippy::all)]
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;

use collab::preclude::{Collab, CollabBuilder, MapExt};
use serde_json::json;

use yrs::updates::decoder::Decode;

use yrs::Map;
use yrs::ReadTxn;

use rand::prelude::SliceRandom;
use rand::thread_rng;
use yrs::types::ToJson;
use yrs::MapRef;
use yrs::Update;

use crate::util::CollabStateCachePlugin;

/// This test is to check if the blocks are deleted randomly, the two clients will have the same document.
///
/// Using state_vector to sync the document.
#[tokio::test]
async fn delete_blocks_randomly_test() {
  let mut client_1 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_1.initialize();
  let mut client_2 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_2.initialize();

  let default_map = json!({
    "block_1": "line 1",
    "block_2": "line 2",
    "block_3": "line 3",
    "block_4": "line 4",
    "block_5": "line 5",
    "block_6": "line 6",
    "block_7": "line 7",
    "block_8": "line 8",
    "block_9": "line 9",
    "block_10": "line 10"
  });

  client_1.data.insert_json_with_path(
    &mut client_1.context.transact_mut(),
    ["map"],
    default_map.clone(),
  );

  client_2
    .data
    .insert_json_with_path(&mut client_2.context.transact_mut(), ["map"], default_map);

  // before delete the blocks
  let map_1_json = client_1.to_json_value();
  let map_2_json = client_2.to_json_value();

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);

  assert_eq!(map_1_json, map_2_json);

  // delete the first 5 blocks
  {
    let mut txn = client_2.context.transact_mut();
    let map_2: MapRef = client_2.data.get_with_path(&txn, ["map"]).unwrap();
    // remove the blocks randomly
    let mut blocks = ["block_1", "block_2", "block_3", "block_4", "block_5"];
    let mut rng = thread_rng();
    blocks.shuffle(&mut rng);
    for block in blocks {
      map_2.remove(&mut txn, block);
    }
  };

  let sv_1 = client_1.transact().state_vector();
  let sv_1_update = client_2.transact().encode_state_as_update_v1(&sv_1);
  let sv_1_update = Update::decode_v1(&sv_1_update).unwrap();
  client_1.apply_update(sv_1_update).unwrap();

  let map_1_json = client_1.to_json_value();
  let map_2_json = client_2.to_json_value();

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);
  println!("map_2_json: {}", map_2_json);

  assert_eq!(map_1_json, map_2_json);
}

/// This test is to check if the blocks are deleted randomly, the two clients will have the same document.
///
/// Using doc_state to sync the document.
#[tokio::test]
async fn delete_blocks_randomly_2_test() {
  let update_cache_1 = CollabStateCachePlugin::new();
  let mut client_1 = CollabBuilder::new(1, "client_1", DataSource::Disk(None))
    .with_device_id("1")
    .with_plugin(update_cache_1.clone())
    .build()
    .unwrap();
  client_1.initialize();

  let update_cache_2 = CollabStateCachePlugin::new();
  let mut client_2 = CollabBuilder::new(2, "client_2", DataSource::Disk(None))
    .with_device_id("2")
    .with_plugin(update_cache_2.clone())
    .build()
    .unwrap();
  client_2.initialize();

  let default_map = json!({
    "block_1": "line 1",
    "block_2": "line 2",
    "block_3": "line 3",
    "block_4": "line 4",
    "block_5": "line 5",
    "block_6": "line 6",
    "block_7": "line 7",
    "block_8": "line 8",
    "block_9": "line 9",
    "block_10": "line 10"
  });

  let _ = client_1
    .data
    .insert_json_with_path(
      &mut client_1.context.transact_mut(),
      ["map"],
      default_map.clone(),
    )
    .unwrap();

  let _ = client_2
    .data
    .insert_json_with_path(&mut client_2.context.transact_mut(), ["map"], default_map)
    .unwrap();

  // before delete the blocks
  let map_1_json = client_1.to_json_value();
  let map_2_json = client_2.to_json_value();

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);

  assert_eq!(map_1_json, map_2_json);

  // delete the first 5 blocks
  {
    let mut txn = client_2.context.transact_mut();
    let map_2: MapRef = client_2.data.get_with_path(&txn, ["map"]).unwrap();
    // remove the blocks randomly
    let mut blocks = ["block_1", "block_2", "block_3", "block_4", "block_5"];
    let mut rng = thread_rng();
    blocks.shuffle(&mut rng);
    for block in blocks {
      map_2.remove(&mut txn, block);
    }
  };

  let _sub1 = client_1.observe_data(move |txn, event| {
    event.target().iter(txn).for_each(|(a, b)| {
      println!("updating -> {}: {}", a, b.to_json(txn));
    });
  });

  let ds_2 = update_cache_2.get_doc_state().unwrap();
  let ds_2_update = ds_2.as_update().unwrap().unwrap();
  client_1.apply_update(ds_2_update).unwrap();

  // wait for the update to be applied
  tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

  let ds_2 = update_cache_2.get_doc_state().unwrap();
  let ds_2_update = ds_2.as_update().unwrap().unwrap();
  client_1.apply_update(ds_2_update).unwrap();

  // wait for the update to be applied
  tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

  // apply the update to client_1
  let map_1_json = client_1.to_json_value();
  let map_2_json = client_2.to_json_value();

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);

  assert_eq!(map_1_json, map_2_json);
}
