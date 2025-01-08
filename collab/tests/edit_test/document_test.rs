#![allow(clippy::all)]
use collab::core::origin::CollabOrigin;

use collab::preclude::{Collab, MapExt};
use serde_json::json;

use yrs::updates::decoder::Decode;

use yrs::Map;
use yrs::ReadTxn;

use rand::prelude::SliceRandom;
use rand::thread_rng;
use yrs::types::ToJson;
use yrs::MapRef;
use yrs::Update;

#[tokio::test]
async fn delete_blocks_randomly_test() {
  let mut client_1 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_1.initialize();
  let mut client_2 = Collab::new_with_origin(CollabOrigin::Empty, "test".to_string(), vec![], true);
  client_2.initialize();

  {
    client_1
      .data
      .insert_json_with_path(
        &mut client_1.context.transact_mut(),
        ["map"],
        json!({
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
        }),
      )
      .unwrap();
  }
  {
    client_2
      .data
      .insert_json_with_path(
        &mut client_2.context.transact_mut(),
        ["map"],
        json!({
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
        }),
      )
      .unwrap();
  }

  // before delete the blocks
  let map_1_json = client_1.to_json_value();
  let map_2_json = client_2.to_json_value();

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);

  let a = map_1_json;
  let b = map_2_json;

  assert_eq!(a, b);

  // delete the first 5 blocks
  let map_2 = {
    let mut txn = client_2.context.transact_mut();
    let map_2: MapRef = client_2.data.get_with_path(&txn, ["map"]).unwrap();
    // remove the blocks randomly
    let mut blocks = ["block_1", "block_2", "block_3", "block_4", "block_5"];
    let mut rng = thread_rng();
    blocks.shuffle(&mut rng);
    for block in blocks {
      map_2.remove(&mut txn, block);
    }
    map_2
  };

  let sv_1 = client_1.transact().state_vector();
  let sv_1_update = client_2.transact().encode_state_as_update_v1(&sv_1);
  let sv_1_update = Update::decode_v1(&sv_1_update).unwrap();

  let map_1: MapRef = {
    client_1.apply_update(sv_1_update).unwrap();

    client_1
      .data
      .get_with_path(&client_1.transact(), ["map"])
      .unwrap()
  };

  let map_1_json = map_1.to_json(&client_1.transact());
  let map_2_json = map_2.to_json(&client_2.transact());

  println!("map_1_json: {}", map_1_json);
  println!("map_2_json: {}", map_2_json);

  let a = map_1.to_json(&client_1.transact());
  let b = map_2.to_json(&client_2.transact());

  assert_eq!(a, b);
}
