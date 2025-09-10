// Document
pub const DOCUMENT_ROOT: &str = "document";

// Folder
pub const FOLDER: &str = "folder";
pub const FOLDER_META: &str = "meta";
pub const FOLDER_WORKSPACE_ID: &str = "current_workspace";

// Database
pub const WORKSPACE_DATABASES: &str = "databases";
pub const DATABASE: &str = "database";
pub const DATABASE_ID: &str = "id";
pub const DATABASE_METAS: &str = "metas";
pub const DATABASE_INLINE_VIEW: &str = "iid";
pub const DATABASE_ROW_DATA: &str = "data";
pub const DATABASE_ROW_ID: &str = "id";

// User Awareness
pub const USER_AWARENESS: &str = "user_awareness";

use uuid::Uuid;

/// Type alias for database ID
pub type DatabaseId = Uuid;

/// Type alias for database view ID
pub type DatabaseViewId = Uuid;

/// Type alias for document ID
pub type DocumentId = Uuid;

/// Type alias for block ID
pub type BlockId = Uuid;

/// Type alias for object ID
pub type ObjectId = Uuid;

/// Type alias for view ID
pub type ViewId = Uuid;

/// Type alias for workspace ID
pub type WorkspaceId = Uuid;

/// Type alias for row ID
pub type RowId = Uuid;
