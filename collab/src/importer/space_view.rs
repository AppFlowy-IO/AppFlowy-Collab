use crate::core::collab::{CollabOptions, DataSource, default_client_id};
use crate::core::origin::CollabOrigin;
use crate::document::document_data::default_document_collab_data;
use crate::error::CollabError;
use crate::folder::hierarchy_builder::{NestedChildViewBuilder, ParentChildViews};
use crate::folder::{SpaceInfo, ViewLayout};
use crate::preclude::Collab;
use uuid::Uuid;

#[allow(dead_code)]
pub fn create_space_view(
  uid: i64,
  workspace_id: &Uuid,
  name: &str,
  view_id: &Uuid,
  child_views: Vec<ParentChildViews>,
  space_info: SpaceInfo,
) -> Result<(ParentChildViews, Collab), CollabError> {
  let client_id = default_client_id();
  let import_container_doc_state = default_document_collab_data(&view_id.to_string(), client_id)
    .map_err(|err| CollabError::Internal(err.into()))?
    .doc_state
    .to_vec();

  let options = CollabOptions::new(*view_id, client_id)
    .with_data_source(DataSource::DocStateV1(import_container_doc_state));
  let collab = Collab::new_with_options(CollabOrigin::Empty, options)
    .map_err(|err| CollabError::Internal(err.into()))?;

  let view = NestedChildViewBuilder::new(uid, *workspace_id)
    .with_view_id(*view_id)
    .with_layout(ViewLayout::Document)
    .with_name(name)
    .with_children(child_views)
    .with_extra(|extra| extra.with_space_info(space_info).build())
    .build();
  Ok((view, collab))
}
