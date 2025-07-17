use collab_entity::CollabType;
use collab_folder::{ViewIcon, ViewLayout};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The relation map of the workspace, including views, collab objects, and dependencies.
///
/// Example:
/// ```json
/// {
///   "workspace_id": "workspace_id_1234567890",
///   "export_timestamp": 1719000000,
///   "views": {
///     "view_id_1234567890": {
///       "view_id": "view_id_1234567890",
///       "name": "View 1",
///       "layout": "Grid",
///       "parent_id": null,
///       "children": [],
///       "collab_object_id": "collab_object_id_1234567890",
///       "created_at": 1719000000,
///       "updated_at": 1719000000
///       "extra": { ... }
///     }
///   },
///   "collab_objects": {
///     "collab_object_id_1234567890": {
///        // document and database have different object id format
///       "object_id": "collab_object_id_database_1234567890",
///       "collab_type": "Database",
///       "size_bytes": 1000,
///     }
///   },
///   "dependencies": []
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRelationMap {
  pub workspace_id: String,
  pub export_timestamp: i64,
  pub views: HashMap<String, ViewMetadata>,
  pub collab_objects: HashMap<String, CollabMetadata>,
  pub dependencies: Vec<ViewDependency>,
  pub workspace_database_meta: Vec<WorkspaceDatabaseMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewMetadata {
  pub view_id: String,
  pub name: String,
  pub layout: ViewLayout,
  pub parent_id: Option<String>,
  pub children: Vec<String>,
  pub collab_object_id: String,
  pub created_at: i64,
  pub updated_at: i64,
  pub extra: Option<String>,
  pub icon: Option<ViewIcon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDatabaseMeta {
  pub database_id: String,
  pub view_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabMetadata {
  pub object_id: String,
  pub collab_type: CollabType,
  pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDependency {
  pub source_view_id: String,
  pub target_view_id: String,
  pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DependencyType {
  // Mention or person
  DocumentReference = 0,
  DatabaseRow = 1,
  DatabaseRelation = 2,
  FileAttachment = 3,
  DatabaseRowDocument = 4,
}
