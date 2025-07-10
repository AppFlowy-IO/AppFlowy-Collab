use collab::core::collab::{CollabOptions, DataSource, default_client_id};
use collab::core::origin::CollabOrigin;
use collab::entity::EncoderVersion;
use collab::preclude::Collab;
use serde_json::json;
use yrs::Update;
use yrs::updates::decoder::Decode;

#[tokio::test]
async fn create_restore_revision() {
  let mut collab = Collab::new(1, "1", "1", default_client_id());
  collab.insert("key", "value1");
  let state1 = collab
    .encode_collab_v1(|_| Ok::<_, anyhow::Error>(()))
    .unwrap();
  let state2 = collab.encode_collab_v2();
  let r1 = collab.create_revision().unwrap();
  collab.insert("key", "value2");

  // revision is equal to the state before the second insert
  let restored = collab.restore_revision(&r1, EncoderVersion::V1).unwrap();
  assert_eq!(restored, state1);

  let restored = collab.restore_revision(&r1, EncoderVersion::V2).unwrap();
  assert_eq!(restored, state2);

  let restored = Collab::new_with_options(
    CollabOrigin::Empty,
    CollabOptions::new("1".into(), default_client_id())
      .with_data_source(DataSource::DocStateV2(restored.doc_state.into())),
  )
  .unwrap();

  // we restored the state before the second insert
  assert_eq!(restored.to_json_value(), json!({"key": "value1"}));
}

#[tokio::test]
async fn remove_revision_cleanups_deleted_data() {
  let mut collab = Collab::new(1, "1", "1", default_client_id());
  collab.insert("key", "value1");
  let r1 = collab.create_named_revision("r1").unwrap();
  collab.insert("key", "value2");
  let r2 = collab.create_named_revision("r2").unwrap();
  collab.insert("key", "value3");
  let r3 = collab.create_named_revision("r3").unwrap();

  let full_state = collab
    .encode_collab_v1(|_| Ok::<_, anyhow::Error>(()))
    .unwrap();

  // removing a middle revision does not clean up the state
  assert!(collab.remove_revision(&r2).unwrap());
  let state = collab
    .encode_collab_v1(|_| Ok::<_, anyhow::Error>(()))
    .unwrap();
  assert!(state.doc_state.len() >= full_state.doc_state.len()); // no data was removed
  assert!(collab.restore_revision(&r2, EncoderVersion::V1).is_err()); // revision no longer exists

  // removing the oldest revision cleans up the state
  assert!(collab.remove_revision(&r1).unwrap());
  let state = collab
    .encode_collab_v1(|_| Ok::<_, anyhow::Error>(()))
    .unwrap();
  assert!(state.doc_state.len() < full_state.doc_state.len());
  assert!(collab.restore_revision(&r1, EncoderVersion::V1).is_err()); // revision no longer exists

  collab.remove_revision(&r3).unwrap();
  println!(
    "{:#?} vs {:#?}",
    Update::decode_v1(&state.doc_state).unwrap(),
    Update::decode_v1(&full_state.doc_state).unwrap()
  );
}
