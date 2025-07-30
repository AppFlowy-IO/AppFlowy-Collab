use crate::util::{create_folder_with_workspace, make_test_view};
use collab_folder::{
  FolderSection, FolderTree, IconType, RepeatedViewIdentifier, UserId, View, ViewIcon,
  ViewIdentifier, ViewLayout,
};
use uuid::Uuid;

// ============================================================================
// Basic Tree Structure Tests
// ============================================================================

#[test]
fn test_folder_tree_basic_hierarchy() {
  let user_id = UserId::from(123);
  let workspace_id = Uuid::new_v4().to_string();

  // Create folder and add views to create a hierarchy:
  // workspace
  //   ├── document_1
  //   └── folder_1
  //       ├── document_2
  //       └── document_3

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  let document_1_id = Uuid::new_v4().to_string();
  let folder_1_id = Uuid::new_v4().to_string();
  let document_2_id = Uuid::new_v4().to_string();
  let document_3_id = Uuid::new_v4().to_string();

  // Create views
  let document_1 = View {
    id: document_1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Document 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "📄".to_string(),
    }),
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let folder_1 = View {
    id: folder_1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Folder 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(document_2_id.clone()),
      ViewIdentifier::new(document_3_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "📁".to_string(),
    }),
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_2 = View {
    id: document_2_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Document 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_3 = View {
    id: document_3_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Document 3".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Grid,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views into folder
  folder_test.folder.insert_view(document_1, None, user_id.as_i64());
  folder_test.folder.insert_view(folder_1, None, user_id.as_i64());
  folder_test.folder.insert_view(document_2, None, user_id.as_i64());
  folder_test.folder.insert_view(document_3, None, user_id.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();

  // Create FolderTree from the exported data
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test root
  assert_eq!(tree.root.id.to_string(), workspace_id);

  // Test parent-child relationships
  let document_1_uuid = Uuid::parse_str(&document_1_id).unwrap();
  let folder_1_uuid = Uuid::parse_str(&folder_1_id).unwrap();
  let document_2_uuid = Uuid::parse_str(&document_2_id).unwrap();
  let document_3_uuid = Uuid::parse_str(&document_3_id).unwrap();
  let workspace_uuid = Uuid::parse_str(&workspace_id).unwrap();

  // Test parents
  assert_eq!(tree.get_parent(&document_1_uuid), Some(&workspace_uuid));
  assert_eq!(tree.get_parent(&folder_1_uuid), Some(&workspace_uuid));
  assert_eq!(tree.get_parent(&document_2_uuid), Some(&folder_1_uuid));
  assert_eq!(tree.get_parent(&document_3_uuid), Some(&folder_1_uuid));
  assert_eq!(tree.get_parent(&workspace_uuid), None); // Root has no parent

  // Test children
  let workspace_children = tree.get_children(&workspace_uuid).unwrap();
  assert_eq!(workspace_children.len(), 2);
  assert!(workspace_children.contains(&document_1_uuid));
  assert!(workspace_children.contains(&folder_1_uuid));

  let folder_1_children = tree.get_children(&folder_1_uuid).unwrap();
  assert_eq!(folder_1_children.len(), 2);
  assert!(folder_1_children.contains(&document_2_uuid));
  assert!(folder_1_children.contains(&document_3_uuid));

  let document_1_children = tree.get_children(&document_1_uuid);
  assert!(document_1_children.is_some());
  assert_eq!(document_1_children.unwrap().len(), 0);

  // Test depths
  assert_eq!(tree.get_depth(&workspace_uuid), 0);
  assert_eq!(tree.get_depth(&document_1_uuid), 1);
  assert_eq!(tree.get_depth(&folder_1_uuid), 1);
  assert_eq!(tree.get_depth(&document_2_uuid), 2);
  assert_eq!(tree.get_depth(&document_3_uuid), 2);

  // Test ancestors
  let document_2_ancestors = tree.get_ancestors(&document_2_uuid);
  assert_eq!(document_2_ancestors.len(), 2);
  assert_eq!(document_2_ancestors[0].id, folder_1_uuid);
  assert_eq!(document_2_ancestors[1].id, workspace_uuid);

  // Test siblings
  let document_2_siblings = tree.get_siblings(&document_2_uuid);
  assert_eq!(document_2_siblings.len(), 1);
  assert_eq!(document_2_siblings[0].id, document_3_uuid);

  let document_1_siblings = tree.get_siblings(&document_1_uuid);
  assert_eq!(document_1_siblings.len(), 1);
  assert_eq!(document_1_siblings[0].id, folder_1_uuid);

  // Test is_ancestor_of
  assert!(tree.is_ancestor_of(&workspace_uuid, &document_2_uuid));
  assert!(tree.is_ancestor_of(&folder_1_uuid, &document_2_uuid));
  assert!(!tree.is_ancestor_of(&document_1_uuid, &document_2_uuid));
  assert!(!tree.is_ancestor_of(&document_2_uuid, &folder_1_uuid));
}

// ============================================================================
// Section Management Tests
// ============================================================================

#[test]
fn test_folder_tree_sections() {
  let user_id = UserId::from(456);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  let favorite_document_id = Uuid::new_v4().to_string();
  let private_document_id = Uuid::new_v4().to_string();
  let normal_document_id = Uuid::new_v4().to_string();
  let trashed_document_id = Uuid::new_v4().to_string();

  // Create views for different sections
  let views = vec![
    View {
      id: favorite_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Favorite Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: true,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: private_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Private Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: normal_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Normal Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: trashed_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Trashed Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
  ];

  // Insert views
  for view in views {
    folder_test.folder.insert_view(view, None, user_id.as_i64());
  }

  // Add views to sections
  folder_test
    .folder
    .add_favorite_view_ids(vec![favorite_document_id.clone()], user_id.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![private_document_id.clone()], user_id.as_i64());
  folder_test
    .folder
    .add_trash_view_ids(vec![trashed_document_id.clone()], user_id.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();

  // Create FolderTree
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test section membership
  assert!(tree.is_in_section(&favorite_document_id, FolderSection::Favorites, user_id.as_i64()));
  assert!(!tree.is_in_section(&private_document_id, FolderSection::Favorites, user_id.as_i64()));
  assert!(tree.is_in_section(&private_document_id, FolderSection::Private, user_id.as_i64()));
  assert!(tree.is_in_section(&trashed_document_id, FolderSection::Trash, user_id.as_i64()));

  // Test section views retrieval
  let favorite_views = tree.get_section_views(FolderSection::Favorites, user_id.as_i64());
  assert_eq!(favorite_views.len(), 1);
  assert_eq!(favorite_views[0].name, "Favorite Document");

  let private_views = tree.get_section_views(FolderSection::Private, user_id.as_i64());
  assert_eq!(private_views.len(), 1);
  assert_eq!(private_views[0].name, "Private Document");

  let trash_views = tree.get_section_views(FolderSection::Trash, user_id.as_i64());
  assert_eq!(trash_views.len(), 1);
  assert_eq!(trash_views[0].name, "Trashed Document");

  // Test section views have correct depth
  let favorite_document_uuid = Uuid::parse_str(&favorite_document_id).unwrap();
  assert_eq!(tree.get_depth(&favorite_document_uuid), 1); // Direct child of workspace
}

// ============================================================================
// Complex Tree Structure Tests
// ============================================================================

#[test]
fn test_folder_tree_complex_hierarchy() {
  let user_id = UserId::from(789);
  let workspace_id = Uuid::new_v4().to_string();

  // Create a more complex hierarchy:
  // workspace
  //   ├── folder_1
  //   │   ├── folder_2
  //   │   │   ├── document_1
  //   │   │   └── document_2
  //   │   └── document_3
  //   └── folder_3
  //       └── folder_4
  //           └── document_4

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  let folder_1_id = Uuid::new_v4().to_string();
  let folder_2_id = Uuid::new_v4().to_string();
  let folder_3_id = Uuid::new_v4().to_string();
  let folder_4_id = Uuid::new_v4().to_string();
  let document_1_id = Uuid::new_v4().to_string();
  let document_2_id = Uuid::new_v4().to_string();
  let document_3_id = Uuid::new_v4().to_string();
  let document_4_id = Uuid::new_v4().to_string();

  // Create all views
  let folder_1 = make_test_view(
    &folder_1_id,
    &workspace_id,
    vec![folder_2_id.clone(), document_3_id.clone()],
  );
  let folder_2 = make_test_view(
    &folder_2_id,
    &folder_1_id,
    vec![document_1_id.clone(), document_2_id.clone()],
  );
  let folder_3 = make_test_view(&folder_3_id, &workspace_id, vec![folder_4_id.clone()]);
  let folder_4 = make_test_view(&folder_4_id, &folder_3_id, vec![document_4_id.clone()]);
  let document_1 = make_test_view(&document_1_id, &folder_2_id, vec![]);
  let document_2 = make_test_view(&document_2_id, &folder_2_id, vec![]);
  let document_3 = make_test_view(&document_3_id, &folder_1_id, vec![]);
  let document_4 = make_test_view(&document_4_id, &folder_4_id, vec![]);

  // Insert all views
  folder_test.folder.insert_view(folder_1, None, user_id.as_i64());
  folder_test.folder.insert_view(folder_2, None, user_id.as_i64());
  folder_test.folder.insert_view(folder_3, None, user_id.as_i64());
  folder_test.folder.insert_view(folder_4, None, user_id.as_i64());
  folder_test.folder.insert_view(document_1, None, user_id.as_i64());
  folder_test.folder.insert_view(document_2, None, user_id.as_i64());
  folder_test.folder.insert_view(document_3, None, user_id.as_i64());
  folder_test.folder.insert_view(document_4, None, user_id.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();

  // Create FolderTree
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test depths
  let document_1_uuid = Uuid::parse_str(&document_1_id).unwrap();
  let document_4_uuid = Uuid::parse_str(&document_4_id).unwrap();
  let folder_2_uuid = Uuid::parse_str(&folder_2_id).unwrap();

  assert_eq!(tree.get_depth(&document_1_uuid), 3); // workspace -> folder_1 -> folder_2 -> document_1
  assert_eq!(tree.get_depth(&document_4_uuid), 3); // workspace -> folder_3 -> folder_4 -> document_4

  // Test path to view
  let path_to_document_1 = tree.get_path_to_view(&document_1_uuid);
  assert_eq!(path_to_document_1.len(), 4);
  assert_eq!(path_to_document_1[0].id.to_string(), workspace_id);
  assert_eq!(path_to_document_1[1].id.to_string(), folder_1_id);
  assert_eq!(path_to_document_1[2].id.to_string(), folder_2_id);
  assert_eq!(path_to_document_1[3].id.to_string(), document_1_id);

  // Test descendants
  let folder_1_uuid = Uuid::parse_str(&folder_1_id).unwrap();
  let folder_1_descendants = tree.get_descendants(&folder_1_uuid);
  assert_eq!(folder_1_descendants.len(), 4); // folder_2, document_1, document_2, document_3

  // Test descendants depths
  for descendant in &folder_1_descendants {
    if descendant.id == folder_2_uuid {
      assert_eq!(tree.get_depth(&descendant.id), 2);
    } else if descendant.id.to_string() == document_1_id || descendant.id.to_string() == document_2_id {
      assert_eq!(tree.get_depth(&descendant.id), 3);
    } else if descendant.id.to_string() == document_3_id {
      assert_eq!(tree.get_depth(&descendant.id), 2);
    }
  }

  // Test all views count
  let all_views = tree.get_all_views();
  assert_eq!(all_views.len(), 9); // workspace + 8 other views
}

// ============================================================================
// View Properties Tests
// ============================================================================

#[test]
fn test_folder_tree_view_properties() {
  let user_id = UserId::from(999);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  let document_id = Uuid::new_v4().to_string();
  let grid_id = Uuid::new_v4().to_string();

  // Create views with different properties
  let document_view = View {
    id: document_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Test Document".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "📝".to_string(),
    }),
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: Some("{\"cover\":{\"type\":\"0\",\"value\":\"#FF0000\"}}".to_string()),
  };

  let grid_view = View {
    id: grid_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Test Grid".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Grid,
    icon: Some(ViewIcon {
      ty: IconType::Url,
      value: "https://example.com/icon.png".to_string(),
    }),
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: Some("{\"line_height_layout\":\"large\"}".to_string()),
  };

  // Insert views
  folder_test.folder.insert_view(document_view, None, user_id.as_i64());
  folder_test.folder.insert_view(grid_view, None, user_id.as_i64());

  // Export and create FolderTree
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test view properties are preserved
  let document_uuid = Uuid::parse_str(&document_id).unwrap();
  let grid_uuid = Uuid::parse_str(&grid_id).unwrap();

  let document_node = tree.get_view(&document_uuid).unwrap();
  assert_eq!(document_node.name, "Test Document");
  assert_eq!(document_node.layout, ViewLayout::Document);
  assert!(document_node.icon.is_some());
  assert_eq!(document_node.icon.as_ref().unwrap().ty, IconType::Emoji);
  assert_eq!(document_node.icon.as_ref().unwrap().value, "📝");
  assert_eq!(
    document_node.extra,
    Some("{\"cover\":{\"type\":\"0\",\"value\":\"#FF0000\"}}".to_string())
  );

  let grid_node = tree.get_view(&grid_uuid).unwrap();
  assert_eq!(grid_node.name, "Test Grid");
  assert_eq!(grid_node.layout, ViewLayout::Grid);
  assert!(grid_node.icon.is_some());
  assert_eq!(grid_node.icon.as_ref().unwrap().ty, IconType::Url);
  assert_eq!(
    grid_node.icon.as_ref().unwrap().value,
    "https://example.com/icon.png"
  );
  assert_eq!(
    grid_node.extra,
    Some("{\"line_height_layout\":\"large\"}".to_string())
  );
}

// ============================================================================
// Multi-User Private Section Tests
// ============================================================================

#[test]
fn test_folder_tree_private_section_multi_user() {
  let user_1_id = UserId::from(1001);
  let user_2_id = UserId::from(1002);
  let _user_3_id = UserId::from(1003);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_1_id.clone(), &workspace_id);

  // Create views owned by different users
  let user_1_private_document_id = Uuid::new_v4().to_string();
  let user_2_private_document_id = Uuid::new_v4().to_string();
  let shared_document_id = Uuid::new_v4().to_string();

  let views = vec![
    View {
      id: user_1_private_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "User1 Private Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_1_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: user_2_private_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "User2 Private Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_2_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: shared_document_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Shared Document".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_1_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
  ];

  // Insert all views
  for view in views {
    folder_test.folder.insert_view(view, None, user_1_id.as_i64());
  }

  // Add private views for each user
  folder_test
    .folder
    .add_private_view_ids(vec![user_1_private_document_id.clone()], user_1_id.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![user_2_private_document_id.clone()], user_2_id.as_i64());

  // Export folder data for user1
  let folder_data_user_1 = folder_test
    .folder
    .get_folder_data(&workspace_id, user_1_id.as_i64())
    .unwrap();
  let tree_user_1 = FolderTree::from_folder_data(folder_data_user_1).unwrap();

  // Export folder data for user2
  let folder_data_user_2 = folder_test
    .folder
    .get_folder_data(&workspace_id, user_2_id.as_i64())
    .unwrap();
  let tree_user_2 = FolderTree::from_folder_data(folder_data_user_2).unwrap();

  // Test that user1 sees only their private view
  assert!(tree_user_1.is_in_section(&user_1_private_document_id, FolderSection::Private, user_1_id.as_i64()));
  assert!(!tree_user_1.is_in_section(&user_2_private_document_id, FolderSection::Private, user_1_id.as_i64()));
  assert!(!tree_user_1.is_in_section(&shared_document_id, FolderSection::Private, user_1_id.as_i64()));

  // Test that user2 sees only their private view
  assert!(!tree_user_2.is_in_section(&user_1_private_document_id, FolderSection::Private, user_2_id.as_i64()));
  assert!(tree_user_2.is_in_section(&user_2_private_document_id, FolderSection::Private, user_2_id.as_i64()));
  assert!(!tree_user_2.is_in_section(&shared_document_id, FolderSection::Private, user_2_id.as_i64()));

  // Check private section views for each user
  let user_1_private_views = tree_user_1.get_section_views(FolderSection::Private, user_1_id.as_i64());
  assert_eq!(user_1_private_views.len(), 1);
  assert_eq!(user_1_private_views[0].name, "User1 Private Document");

  let user_2_private_views = tree_user_2.get_section_views(FolderSection::Private, user_2_id.as_i64());
  assert_eq!(user_2_private_views.len(), 1);
  assert_eq!(user_2_private_views[0].name, "User2 Private Document");
}

#[test]
fn test_folder_tree_private_section_nested_views() {
  let user_id = UserId::from(2001);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  // Create a private folder with children
  let private_folder_id = Uuid::new_v4().to_string();
  let child_1_id = Uuid::new_v4().to_string();
  let child_2_id = Uuid::new_v4().to_string();
  let nested_child_id = Uuid::new_v4().to_string();

  let private_folder = View {
    id: private_folder_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Private Folder".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(child_1_id.clone()),
      ViewIdentifier::new(child_2_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "🔒".to_string(),
    }),
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let child_1 = View {
    id: child_1_id.clone(),
    parent_view_id: private_folder_id.clone(),
    name: "Private Child 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(nested_child_id.clone())]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let child_2 = View {
    id: child_2_id.clone(),
    parent_view_id: private_folder_id.clone(),
    name: "Private Child 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Grid,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let nested_child = View {
    id: nested_child_id.clone(),
    parent_view_id: child_1_id.clone(),
    name: "Nested Private Child".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views
  folder_test.folder.insert_view(private_folder, None, user_id.as_i64());
  folder_test.folder.insert_view(child_1, None, user_id.as_i64());
  folder_test.folder.insert_view(child_2, None, user_id.as_i64());
  folder_test.folder.insert_view(nested_child, None, user_id.as_i64());

  // Add only the parent folder to private section
  folder_test
    .folder
    .add_private_view_ids(vec![private_folder_id.clone()], user_id.as_i64());

  // Export and create FolderTree
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test that only the parent is in private section, not its children
  assert!(tree.is_in_section(&private_folder_id, FolderSection::Private, user_id.as_i64()));
  assert!(!tree.is_in_section(&child_1_id, FolderSection::Private, user_id.as_i64()));
  assert!(!tree.is_in_section(&child_2_id, FolderSection::Private, user_id.as_i64()));
  assert!(!tree.is_in_section(&nested_child_id, FolderSection::Private, user_id.as_i64()));

  // Test private section views
  let private_views = tree.get_section_views(FolderSection::Private, user_id.as_i64());
  assert_eq!(private_views.len(), 1);
  assert_eq!(private_views[0].name, "Private Folder");

  let private_folder_uuid = Uuid::parse_str(&private_folder_id).unwrap();
  assert_eq!(tree.get_depth(&private_folder_uuid), 1); // Direct child of workspace

  // Test that we can still navigate to children of private views
  let private_folder_children = tree.get_children(&private_folder_uuid).unwrap();
  assert_eq!(private_folder_children.len(), 2);

  // Test descendants of private folder
  let descendants = tree.get_descendants(&private_folder_uuid);
  assert_eq!(descendants.len(), 3); // child_1, child_2, nested_child
}

#[test]
fn test_folder_tree_private_section_mixed_ownership() {
  let user_1_id = UserId::from(3001);
  let user_2_id = UserId::from(3002);
  let user_3_id = UserId::from(3003);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_1_id.clone(), &workspace_id);

  // Create views with mixed ownership
  let document_1_id = Uuid::new_v4().to_string();
  let document_2_id = Uuid::new_v4().to_string();
  let document_3_id = Uuid::new_v4().to_string();
  let document_4_id = Uuid::new_v4().to_string();

  // User1's document
  folder_test.folder.insert_view(
    View {
      id: document_1_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Document 1 by User1".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_1_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(user_1_id.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    user_1_id.as_i64(),
  );

  // User2's document
  folder_test.folder.insert_view(
    View {
      id: document_2_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Document 2 by User2".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_2_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(user_2_id.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    user_2_id.as_i64(),
  );

  // Document created by User1 but edited by User2
  folder_test.folder.insert_view(
    View {
      id: document_3_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Document 3 created by User1, edited by User2".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_1_id.as_i64()),
      last_edited_time: 100,
      last_edited_by: Some(user_2_id.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    user_1_id.as_i64(),
  );

  // User3's document
  folder_test.folder.insert_view(
    View {
      id: document_4_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Document 4 by User3".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(user_3_id.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(user_3_id.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    user_3_id.as_i64(),
  );

  // Each user marks different documents as private
  folder_test
    .folder
    .add_private_view_ids(vec![document_1_id.clone(), document_3_id.clone()], user_1_id.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![document_2_id.clone(), document_3_id.clone()], user_2_id.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![document_4_id.clone()], user_3_id.as_i64());

  // Test User1's view
  let folder_data_user_1 = folder_test
    .folder
    .get_folder_data(&workspace_id, user_1_id.as_i64())
    .unwrap();
  let tree_user_1 = FolderTree::from_folder_data(folder_data_user_1).unwrap();

  let user_1_private_views = tree_user_1.get_section_views(FolderSection::Private, user_1_id.as_i64());
  assert_eq!(user_1_private_views.len(), 2);
  let private_names: Vec<&str> = user_1_private_views.iter().map(|v| v.name.as_str()).collect();
  assert!(private_names.contains(&"Document 1 by User1"));
  assert!(private_names.contains(&"Document 3 created by User1, edited by User2"));

  // Test User2's view
  let folder_data_user_2 = folder_test
    .folder
    .get_folder_data(&workspace_id, user_2_id.as_i64())
    .unwrap();
  let tree_user_2 = FolderTree::from_folder_data(folder_data_user_2).unwrap();

  let user_2_private_views = tree_user_2.get_section_views(FolderSection::Private, user_2_id.as_i64());
  assert_eq!(user_2_private_views.len(), 2);
  let private_names: Vec<&str> = user_2_private_views.iter().map(|v| v.name.as_str()).collect();
  assert!(private_names.contains(&"Document 2 by User2"));
  assert!(private_names.contains(&"Document 3 created by User1, edited by User2"));

  // Test User3's view
  let folder_data_user_3 = folder_test
    .folder
    .get_folder_data(&workspace_id, user_3_id.as_i64())
    .unwrap();
  let tree_user_3 = FolderTree::from_folder_data(folder_data_user_3).unwrap();

  let user_3_private_views = tree_user_3.get_section_views(FolderSection::Private, user_3_id.as_i64());
  assert_eq!(user_3_private_views.len(), 1);
  assert_eq!(user_3_private_views[0].name, "Document 4 by User3");

  // Verify that document_3 is private for both user1 and user2
  assert!(tree_user_1.is_in_section(&document_3_id, FolderSection::Private, user_1_id.as_i64()));
  assert!(tree_user_2.is_in_section(&document_3_id, FolderSection::Private, user_2_id.as_i64()));
  assert!(!tree_user_3.is_in_section(&document_3_id, FolderSection::Private, user_3_id.as_i64()));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_folder_tree_private_section_empty() {
  let user_id = UserId::from(4001);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  // Create some views but don't mark any as private
  let document_1_id = Uuid::new_v4().to_string();
  let document_2_id = Uuid::new_v4().to_string();

  folder_test.folder.insert_view(
    make_test_view(&document_1_id, &workspace_id, vec![]),
    None,
    user_id.as_i64(),
  );

  folder_test.folder.insert_view(
    make_test_view(&document_2_id, &workspace_id, vec![]),
    None,
    user_id.as_i64(),
  );

  // Export without adding any private views
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test that private section is empty
  let private_views = tree.get_section_views(FolderSection::Private, user_id.as_i64());
  assert_eq!(private_views.len(), 0);

  // Verify no views are in private section
  assert!(!tree.is_in_section(&document_1_id, FolderSection::Private, user_id.as_i64()));
  assert!(!tree.is_in_section(&document_2_id, FolderSection::Private, user_id.as_i64()));
}

// ============================================================================
// Advanced Tree Operations Tests
// ============================================================================

#[test]
fn test_folder_tree_descendants_with_depth_limit() {
  let user_id = UserId::from(5001);
  let workspace_id = Uuid::new_v4().to_string();

  // Create a deeper hierarchy:
  // workspace
  //   └── folder_1
  //       ├── document_1
  //       └── subfolder
  //           └── subdocument

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  let folder_1_id = Uuid::new_v4().to_string();
  let document_1_id = Uuid::new_v4().to_string();
  let subfolder_id = Uuid::new_v4().to_string();
  let subdocument_id = Uuid::new_v4().to_string();

  let folder_1 = View {
    id: folder_1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Folder 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(document_1_id.clone()),
      ViewIdentifier::new(subfolder_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_1 = View {
    id: document_1_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Document 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let subfolder = View {
    id: subfolder_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Subfolder".to_string(),
    children: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(subdocument_id.clone())]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let subdocument = View {
    id: subdocument_id.clone(),
    parent_view_id: subfolder_id.clone(),
    name: "Sub Document".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views
  folder_test.folder.insert_view(folder_1, None, user_id.as_i64());
  folder_test.folder.insert_view(document_1, None, user_id.as_i64());
  folder_test.folder.insert_view(subfolder, None, user_id.as_i64());
  folder_test.folder.insert_view(subdocument, None, user_id.as_i64());

  // Export and create FolderTree
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  let folder_1_uuid = Uuid::parse_str(&folder_1_id).unwrap();

  // Test depth = 0: no descendants
  let descendants_depth_0 = tree.get_descendants_with_children(&folder_1_uuid, 0);
  assert_eq!(descendants_depth_0.len(), 0);

  // Test depth = 1: only direct children (document_1 and subfolder)
  let descendants_depth_1 = tree.get_descendants_with_children(&folder_1_uuid, 1);
  assert_eq!(descendants_depth_1.len(), 2);
  let names_depth_1: Vec<String> = descendants_depth_1.iter().map(|d| d.view.name.clone()).collect();
  assert!(names_depth_1.contains(&"Document 1".to_string()));
  assert!(names_depth_1.contains(&"Subfolder".to_string()));

  // Test depth = 2: children and grandchildren (document_1, subfolder, subdocument)
  let descendants_depth_2 = tree.get_descendants_with_children(&folder_1_uuid, 2);
  assert_eq!(descendants_depth_2.len(), 3);

  // Find each descendant and verify its properties
  let document_1_descendant = descendants_depth_2
    .iter()
    .find(|d| d.view.name == "Document 1")
    .unwrap();
  assert_eq!(document_1_descendant.children.len(), 0); // No children

  let subfolder_descendant = descendants_depth_2
    .iter()
    .find(|d| d.view.name == "Subfolder")
    .unwrap();
  assert_eq!(subfolder_descendant.children.len(), 1); // Has subdocument as child

  let subdocument_descendant = descendants_depth_2
    .iter()
    .find(|d| d.view.name == "Sub Document")
    .unwrap();
  assert_eq!(subdocument_descendant.children.len(), 0); // No children

  // Test from workspace with different depths
  let workspace_uuid = Uuid::parse_str(&workspace_id).unwrap();

  // depth = 1: only folder_1
  let workspace_descendants_depth_1 = tree.get_descendants_with_children(&workspace_uuid, 1);
  assert_eq!(workspace_descendants_depth_1.len(), 1);
  assert_eq!(workspace_descendants_depth_1[0].view.name, "Folder 1");

  // depth = 2: folder_1, document_1, subfolder
  let workspace_descendants_depth_2 = tree.get_descendants_with_children(&workspace_uuid, 2);
  assert_eq!(workspace_descendants_depth_2.len(), 3);

  // depth = 3: all views
  let workspace_descendants_depth_3 = tree.get_descendants_with_children(&workspace_uuid, 3);
  assert_eq!(workspace_descendants_depth_3.len(), 4);
}

// ============================================================================
// Cycle Detection Tests
// ============================================================================

#[test]
fn test_folder_tree_cycle_detection_normal_tree() {
  let user_id = UserId::from(8888);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  // Create a normal tree structure without cycles
  let folder_1_id = Uuid::new_v4().to_string();
  let folder_2_id = Uuid::new_v4().to_string();
  let document_1_id = Uuid::new_v4().to_string();
  let document_2_id = Uuid::new_v4().to_string();

  let folder_1 = View {
    id: folder_1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Folder 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(document_1_id.clone()),
      ViewIdentifier::new(folder_2_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let folder_2 = View {
    id: folder_2_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Folder 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(document_2_id.clone())]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_1 = View {
    id: document_1_id.clone(),
    parent_view_id: folder_1_id.clone(),
    name: "Document 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_2 = View {
    id: document_2_id.clone(),
    parent_view_id: folder_2_id.clone(),
    name: "Document 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views
  folder_test.folder.insert_view(folder_1, None, user_id.as_i64());
  folder_test.folder.insert_view(folder_2, None, user_id.as_i64());
  folder_test.folder.insert_view(document_1, None, user_id.as_i64());
  folder_test.folder.insert_view(document_2, None, user_id.as_i64());

  // Export and create FolderTree
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test that normal tree has no cycles
  assert!(!tree.has_cycles(), "Normal tree should not have cycles");

  // Test that we can get ancestors without issues
  let document_2_uuid = Uuid::parse_str(&document_2_id).unwrap();
  let ancestors = tree.get_ancestors(&document_2_uuid);
  assert_eq!(ancestors.len(), 3); // workspace, folder_1, folder_2

  // Test that we can get depth correctly
  let document_2_depth = tree.get_depth(&document_2_uuid);
  assert_eq!(document_2_depth, 3); // workspace -> folder_1 -> folder_2 -> document_2

  // Test ancestor relationships
  let folder_1_uuid = Uuid::parse_str(&folder_1_id).unwrap();
  let folder_2_uuid = Uuid::parse_str(&folder_2_id).unwrap();
  
  assert!(tree.is_ancestor_of(&folder_1_uuid, &document_2_uuid));
  assert!(tree.is_ancestor_of(&folder_2_uuid, &document_2_uuid));
  assert!(!tree.is_ancestor_of(&document_2_uuid, &folder_1_uuid));
}

// ============================================================================
// Safety and Edge Case Tests
// ============================================================================

#[test]
fn test_folder_tree_potential_issues() {
  let user_id = UserId::from(9999);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(user_id.clone(), &workspace_id);

  // Test 1: Self-referencing parent (should be handled gracefully)
  let self_referencing_document_id = Uuid::new_v4().to_string();
  let self_referencing_view = View {
    id: self_referencing_document_id.clone(),
    parent_view_id: self_referencing_document_id.clone(), // Self-referencing!
    name: "Self Referencing Document".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  folder_test.folder.insert_view(self_referencing_view, None, user_id.as_i64());

  // Test 2: Circular reference (A -> B -> A)
  let document_a_id = Uuid::new_v4().to_string();
  let document_b_id = Uuid::new_v4().to_string();
  
  let document_a = View {
    id: document_a_id.clone(),
    parent_view_id: document_b_id.clone(), // A's parent is B
    name: "Document A".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let document_b = View {
    id: document_b_id.clone(),
    parent_view_id: document_a_id.clone(), // B's parent is A - circular!
    name: "Document B".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  folder_test.folder.insert_view(document_a, None, user_id.as_i64());
  folder_test.folder.insert_view(document_b, None, user_id.as_i64());

  // Test 3: Orphaned view (parent doesn't exist)
  let orphaned_document_id = Uuid::new_v4().to_string();
  let non_existent_parent_id = Uuid::new_v4().to_string();
  let orphaned_view = View {
    id: orphaned_document_id.clone(),
    parent_view_id: non_existent_parent_id, // Parent doesn't exist
    name: "Orphaned Document".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(user_id.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  folder_test.folder.insert_view(orphaned_view, None, user_id.as_i64());

  // Export and create FolderTree
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, user_id.as_i64())
    .unwrap();
  
  let tree = FolderTree::from_folder_data(folder_data).unwrap();

  // Test that tree creation didn't panic
  assert_eq!(tree.root.id.to_string(), workspace_id);

  // Test that we can get views without infinite loops
  let self_referencing_uuid = Uuid::parse_str(&self_referencing_document_id).unwrap();
  let document_a_uuid = Uuid::parse_str(&document_a_id).unwrap();
  let document_b_uuid = Uuid::parse_str(&document_b_id).unwrap();
  let orphaned_uuid = Uuid::parse_str(&orphaned_document_id).unwrap();

  // These should not cause infinite loops and should complete quickly
  let self_referencing_ancestors = tree.get_ancestors(&self_referencing_uuid);
  let document_a_ancestors = tree.get_ancestors(&document_a_uuid);
  let document_b_ancestors = tree.get_ancestors(&document_b_uuid);
  let orphaned_ancestors = tree.get_ancestors(&orphaned_uuid);

  // Test that we can get depth without infinite loops
  let self_referencing_depth = tree.get_depth(&self_referencing_uuid);
  let document_a_depth = tree.get_depth(&document_a_uuid);
  let document_b_depth = tree.get_depth(&document_b_uuid);
  let orphaned_depth = tree.get_depth(&orphaned_uuid);

  // Test that we can check ancestor relationships without infinite loops
  let is_a_ancestor_of_b = tree.is_ancestor_of(&document_a_uuid, &document_b_uuid);
  let is_b_ancestor_of_a = tree.is_ancestor_of(&document_b_uuid, &document_a_uuid);

  // Test cycle detection
  let has_cycles = tree.has_cycles();

  // Print debug info
  println!("Self-referencing ancestors count: {}", self_referencing_ancestors.len());
  println!("Document A ancestors count: {}", document_a_ancestors.len());
  println!("Document B ancestors count: {}", document_b_ancestors.len());
  println!("Orphaned ancestors count: {}", orphaned_ancestors.len());
  println!("Self-referencing depth: {}", self_referencing_depth);
  println!("Document A depth: {}", document_a_depth);
  println!("Document B depth: {}", document_b_depth);
  println!("Orphaned depth: {}", orphaned_depth);
  println!("Is A ancestor of B: {}", is_a_ancestor_of_b);
  println!("Is B ancestor of A: {}", is_b_ancestor_of_a);
  println!("Has cycles: {}", has_cycles);

  // Basic assertions to ensure the tree is functional
  let all_views = tree.get_all_views();
  assert!(all_views.len() > 0);
  
  // The tree should handle these edge cases gracefully
  // We expect cycles to be detected
  assert!(has_cycles, "Tree should detect cycles");
  
  // Self-referencing should result in limited ancestors (due to cycle detection)
  assert!(self_referencing_ancestors.len() < 1000, "Self-reference should be limited by cycle detection");
  
  // Circular references should result in limited ancestors
  assert!(document_a_ancestors.len() < 1000, "Circular reference should be limited by cycle detection");
  assert!(document_b_ancestors.len() < 1000, "Circular reference should be limited by cycle detection");
}
