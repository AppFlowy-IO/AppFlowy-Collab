use crate::error::ImporterError;
use collab::core::collab::DataSource;
use collab::core::origin::CollabOrigin;
use collab::preclude::Collab;
use collab_document::document_data::default_document_collab_data;
use collab_folder::hierarchy_builder::{NestedChildViewBuilder, ParentChildViews};
use collab_folder::{SpaceInfo, ViewLayout};

#[allow(dead_code)]
pub fn create_space_view(
  uid: i64,
  workspace_id: &str,
  name: &str,
  view_id: &str,
  child_views: Vec<ParentChildViews>,
  space_info: SpaceInfo,
) -> Result<(ParentChildViews, Collab), ImporterError> {
  let import_container_doc_state = default_document_collab_data(view_id)
    .map_err(|err| ImporterError::Internal(err.into()))?
    .doc_state
    .to_vec();

  let collab = Collab::new_with_source(
    CollabOrigin::Empty,
    view_id,
    DataSource::DocStateV1(import_container_doc_state),
    None,
  )
  .map_err(|err| ImporterError::Internal(err.into()))?;

  let view = NestedChildViewBuilder::new(uid, workspace_id.to_string())
    .with_view_id(view_id)
    .with_layout(ViewLayout::Document)
    .with_name(name)
    .with_children(child_views)
    .with_extra(|extra| extra.with_space_info(space_info).build())
    .build();
  Ok((view, collab))
}
