use collab::entity::EncodedCollab;
use collab_entity::CollabType;

pub struct ImportedCollabView {
  pub name: String,
  pub imported_type: ImportedType,
  pub collabs: Vec<ImportedCollab>,
}

pub struct ImportedCollab {
  pub object_id: String,
  pub collab_type: CollabType,
  pub encoded_collab: EncodedCollab,
}

pub enum ImportedType {
  Document,
  Database,
}
