use crate::util::{create_folder_with_workspace, make_test_view};
use collab_folder::{
  FolderHierarchy, FolderSection, IconType, RepeatedViewIdentifier, UserId, View, ViewIcon,
  ViewIdentifier, ViewLayout,
};
use uuid::Uuid;

#[test]
fn test_folder_hierarchy_basic_parent_child() {
  let uid = UserId::from(123);
  let workspace_id = Uuid::new_v4().to_string();

  // Create folder and add views to create a hierarchy:
  // workspace
  //   ├── doc1
  //   └── folder1
  //       ├── doc2
  //       └── doc3

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  let doc1_id = Uuid::new_v4().to_string();
  let folder1_id = Uuid::new_v4().to_string();
  let doc2_id = Uuid::new_v4().to_string();
  let doc3_id = Uuid::new_v4().to_string();

  // Create views
  let doc1 = View {
    id: doc1_id.clone(),
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
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let folder1 = View {
    id: folder1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Folder 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(doc2_id.clone()),
      ViewIdentifier::new(doc3_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "📁".to_string(),
    }),
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let doc2 = View {
    id: doc2_id.clone(),
    parent_view_id: folder1_id.clone(),
    name: "Document 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let doc3 = View {
    id: doc3_id.clone(),
    parent_view_id: folder1_id.clone(),
    name: "Document 3".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Grid,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views into folder
  folder_test.folder.insert_view(doc1, None, uid.as_i64());
  folder_test.folder.insert_view(folder1, None, uid.as_i64());
  folder_test.folder.insert_view(doc2, None, uid.as_i64());
  folder_test.folder.insert_view(doc3, None, uid.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();

  // Create FolderHierarchy from the exported data
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test root
  assert_eq!(hierarchy.root.id.to_string(), workspace_id);

  // Test parent-child relationships
  let doc1_uuid = Uuid::parse_str(&doc1_id).unwrap();
  let folder1_uuid = Uuid::parse_str(&folder1_id).unwrap();
  let doc2_uuid = Uuid::parse_str(&doc2_id).unwrap();
  let doc3_uuid = Uuid::parse_str(&doc3_id).unwrap();
  let workspace_uuid = Uuid::parse_str(&workspace_id).unwrap();

  // Test parents
  assert_eq!(hierarchy.get_parent(&doc1_uuid), Some(&workspace_uuid));
  assert_eq!(hierarchy.get_parent(&folder1_uuid), Some(&workspace_uuid));
  assert_eq!(hierarchy.get_parent(&doc2_uuid), Some(&folder1_uuid));
  assert_eq!(hierarchy.get_parent(&doc3_uuid), Some(&folder1_uuid));
  assert_eq!(hierarchy.get_parent(&workspace_uuid), None); // Root has no parent

  // Test children
  let workspace_children = hierarchy.get_children(&workspace_uuid).unwrap();
  assert_eq!(workspace_children.len(), 2);
  assert!(workspace_children.contains(&doc1_uuid));
  assert!(workspace_children.contains(&folder1_uuid));

  let folder1_children = hierarchy.get_children(&folder1_uuid).unwrap();
  assert_eq!(folder1_children.len(), 2);
  assert!(folder1_children.contains(&doc2_uuid));
  assert!(folder1_children.contains(&doc3_uuid));

  let doc1_children = hierarchy.get_children(&doc1_uuid);
  assert!(doc1_children.is_some());
  assert_eq!(doc1_children.unwrap().len(), 0);

  // Test depths
  assert_eq!(hierarchy.get_depth(&workspace_uuid), 0);
  assert_eq!(hierarchy.get_depth(&doc1_uuid), 1);
  assert_eq!(hierarchy.get_depth(&folder1_uuid), 1);
  assert_eq!(hierarchy.get_depth(&doc2_uuid), 2);
  assert_eq!(hierarchy.get_depth(&doc3_uuid), 2);

  // Test ancestors
  let doc2_ancestors = hierarchy.get_ancestors(&doc2_uuid);
  assert_eq!(doc2_ancestors.len(), 2);
  assert_eq!(doc2_ancestors[0].id, folder1_uuid);
  assert_eq!(doc2_ancestors[1].id, workspace_uuid);

  // Test siblings
  let doc2_siblings = hierarchy.get_siblings(&doc2_uuid);
  assert_eq!(doc2_siblings.len(), 1);
  assert_eq!(doc2_siblings[0].id, doc3_uuid);

  let doc1_siblings = hierarchy.get_siblings(&doc1_uuid);
  assert_eq!(doc1_siblings.len(), 1);
  assert_eq!(doc1_siblings[0].id, folder1_uuid);

  // Test is_ancestor_of
  assert!(hierarchy.is_ancestor_of(&workspace_uuid, &doc2_uuid));
  assert!(hierarchy.is_ancestor_of(&folder1_uuid, &doc2_uuid));
  assert!(!hierarchy.is_ancestor_of(&doc1_uuid, &doc2_uuid));
  assert!(!hierarchy.is_ancestor_of(&doc2_uuid, &folder1_uuid));
}

#[test]
fn test_folder_hierarchy_with_sections() {
  let uid = UserId::from(456);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  let doc1_id = Uuid::new_v4().to_string();
  let doc2_id = Uuid::new_v4().to_string();
  let doc3_id = Uuid::new_v4().to_string();
  let trash_doc_id = Uuid::new_v4().to_string();

  // Create views
  let views = vec![
    View {
      id: doc1_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Favorite Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: true,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: doc2_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Private Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: doc3_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Normal Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: trash_doc_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Trashed Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
  ];

  // Insert views
  for view in views {
    folder_test.folder.insert_view(view, None, uid.as_i64());
  }

  // Add to sections
  folder_test
    .folder
    .add_favorite_view_ids(vec![doc1_id.clone()], uid.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![doc2_id.clone()], uid.as_i64());
  folder_test
    .folder
    .add_trash_view_ids(vec![trash_doc_id.clone()], uid.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();

  // Create FolderHierarchy
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test sections
  assert!(hierarchy.is_in_section(&doc1_id, FolderSection::Favorites));
  assert!(!hierarchy.is_in_section(&doc2_id, FolderSection::Favorites));
  assert!(hierarchy.is_in_section(&doc2_id, FolderSection::Private));
  assert!(hierarchy.is_in_section(&trash_doc_id, FolderSection::Trash));

  // Test get section views
  let favorites = hierarchy.get_section_views(FolderSection::Favorites);
  assert_eq!(favorites.len(), 1);
  assert_eq!(favorites[0].name, "Favorite Doc");

  let private_views = hierarchy.get_section_views(FolderSection::Private);
  assert_eq!(private_views.len(), 1);
  assert_eq!(private_views[0].name, "Private Doc");

  let trash_views = hierarchy.get_section_views(FolderSection::Trash);
  assert_eq!(trash_views.len(), 1);
  assert_eq!(trash_views[0].name, "Trashed Doc");

  // Test section views have correct depth
  let doc1_uuid = Uuid::parse_str(&doc1_id).unwrap();
  assert_eq!(hierarchy.get_depth(&doc1_uuid), 1); // Direct child of workspace
}

#[test]
fn test_folder_hierarchy_complex_tree() {
  let uid = UserId::from(789);
  let workspace_id = Uuid::new_v4().to_string();

  // Create a more complex hierarchy:
  // workspace
  //   ├── folder1
  //   │   ├── folder2
  //   │   │   ├── doc1
  //   │   │   └── doc2
  //   │   └── doc3
  //   └── folder3
  //       └── folder4
  //           └── doc4

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  let folder1_id = Uuid::new_v4().to_string();
  let folder2_id = Uuid::new_v4().to_string();
  let folder3_id = Uuid::new_v4().to_string();
  let folder4_id = Uuid::new_v4().to_string();
  let doc1_id = Uuid::new_v4().to_string();
  let doc2_id = Uuid::new_v4().to_string();
  let doc3_id = Uuid::new_v4().to_string();
  let doc4_id = Uuid::new_v4().to_string();

  // Create all views
  let folder1 = make_test_view(
    &folder1_id,
    &workspace_id,
    vec![folder2_id.clone(), doc3_id.clone()],
  );
  let folder2 = make_test_view(
    &folder2_id,
    &folder1_id,
    vec![doc1_id.clone(), doc2_id.clone()],
  );
  let folder3 = make_test_view(&folder3_id, &workspace_id, vec![folder4_id.clone()]);
  let folder4 = make_test_view(&folder4_id, &folder3_id, vec![doc4_id.clone()]);
  let doc1 = make_test_view(&doc1_id, &folder2_id, vec![]);
  let doc2 = make_test_view(&doc2_id, &folder2_id, vec![]);
  let doc3 = make_test_view(&doc3_id, &folder1_id, vec![]);
  let doc4 = make_test_view(&doc4_id, &folder4_id, vec![]);

  // Insert all views
  folder_test.folder.insert_view(folder1, None, uid.as_i64());
  folder_test.folder.insert_view(folder2, None, uid.as_i64());
  folder_test.folder.insert_view(folder3, None, uid.as_i64());
  folder_test.folder.insert_view(folder4, None, uid.as_i64());
  folder_test.folder.insert_view(doc1, None, uid.as_i64());
  folder_test.folder.insert_view(doc2, None, uid.as_i64());
  folder_test.folder.insert_view(doc3, None, uid.as_i64());
  folder_test.folder.insert_view(doc4, None, uid.as_i64());

  // Export to FolderData
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();

  // Create hierarchy
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test depths
  let doc1_uuid = Uuid::parse_str(&doc1_id).unwrap();
  let doc4_uuid = Uuid::parse_str(&doc4_id).unwrap();
  let folder2_uuid = Uuid::parse_str(&folder2_id).unwrap();

  assert_eq!(hierarchy.get_depth(&doc1_uuid), 3); // workspace -> folder1 -> folder2 -> doc1
  assert_eq!(hierarchy.get_depth(&doc4_uuid), 3); // workspace -> folder3 -> folder4 -> doc4

  // Test path to view
  let path_to_doc1 = hierarchy.get_path_to_view(&doc1_uuid);
  assert_eq!(path_to_doc1.len(), 4);
  assert_eq!(path_to_doc1[0].id.to_string(), workspace_id);
  assert_eq!(path_to_doc1[1].id.to_string(), folder1_id);
  assert_eq!(path_to_doc1[2].id.to_string(), folder2_id);
  assert_eq!(path_to_doc1[3].id.to_string(), doc1_id);

  // Test descendants
  let folder1_uuid = Uuid::parse_str(&folder1_id).unwrap();
  let folder1_descendants = hierarchy.get_descendants(&folder1_uuid);
  assert_eq!(folder1_descendants.len(), 4); // folder2, doc1, doc2, doc3

  // Test descendants depths
  for desc in &folder1_descendants {
    if desc.id == folder2_uuid {
      assert_eq!(hierarchy.get_depth(&desc.id), 2);
    } else if desc.id.to_string() == doc1_id || desc.id.to_string() == doc2_id {
      assert_eq!(hierarchy.get_depth(&desc.id), 3);
    } else if desc.id.to_string() == doc3_id {
      assert_eq!(hierarchy.get_depth(&desc.id), 2);
    }
  }

  // Test all views count
  let all_views = hierarchy.get_all_views();
  assert_eq!(all_views.len(), 9); // workspace + 8 other views
}

#[test]
fn test_folder_hierarchy_view_properties() {
  let uid = UserId::from(999);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  let doc_id = Uuid::new_v4().to_string();
  let grid_id = Uuid::new_v4().to_string();

  // Create views with different properties
  let doc_view = View {
    id: doc_id.clone(),
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
    created_by: Some(uid.as_i64()),
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
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: Some("{\"line_height_layout\":\"large\"}".to_string()),
  };

  // Insert views
  folder_test.folder.insert_view(doc_view, None, uid.as_i64());
  folder_test
    .folder
    .insert_view(grid_view, None, uid.as_i64());

  // Export and create hierarchy
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test view properties are preserved
  let doc_uuid = Uuid::parse_str(&doc_id).unwrap();
  let grid_uuid = Uuid::parse_str(&grid_id).unwrap();

  let doc_hierarchy_view = hierarchy.get_view(&doc_uuid).unwrap();
  assert_eq!(doc_hierarchy_view.name, "Test Document");
  assert_eq!(doc_hierarchy_view.layout, ViewLayout::Document);
  assert!(doc_hierarchy_view.icon.is_some());
  assert_eq!(
    doc_hierarchy_view.icon.as_ref().unwrap().ty,
    IconType::Emoji
  );
  assert_eq!(doc_hierarchy_view.icon.as_ref().unwrap().value, "📝");
  assert_eq!(
    doc_hierarchy_view.extra,
    Some("{\"cover\":{\"type\":\"0\",\"value\":\"#FF0000\"}}".to_string())
  );

  let grid_hierarchy_view = hierarchy.get_view(&grid_uuid).unwrap();
  assert_eq!(grid_hierarchy_view.name, "Test Grid");
  assert_eq!(grid_hierarchy_view.layout, ViewLayout::Grid);
  assert!(grid_hierarchy_view.icon.is_some());
  assert_eq!(grid_hierarchy_view.icon.as_ref().unwrap().ty, IconType::Url);
  assert_eq!(
    grid_hierarchy_view.icon.as_ref().unwrap().value,
    "https://example.com/icon.png"
  );
  assert_eq!(
    grid_hierarchy_view.extra,
    Some("{\"line_height_layout\":\"large\"}".to_string())
  );
}

#[test]
fn test_folder_hierarchy_private_section_multi_user() {
  let uid1 = UserId::from(1001);
  let uid2 = UserId::from(1002);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid1.clone(), &workspace_id);

  // Create views owned by different users
  let user1_private_doc = Uuid::new_v4().to_string();
  let user2_private_doc = Uuid::new_v4().to_string();
  let shared_doc = Uuid::new_v4().to_string();

  let views = vec![
    View {
      id: user1_private_doc.clone(),
      parent_view_id: workspace_id.clone(),
      name: "User1 Private Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid1.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: user2_private_doc.clone(),
      parent_view_id: workspace_id.clone(),
      name: "User2 Private Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid2.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
    View {
      id: shared_doc.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Shared Doc".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid1.as_i64()),
      last_edited_time: 0,
      last_edited_by: None,
      is_locked: None,
      extra: None,
    },
  ];

  // Insert all views
  for view in views {
    folder_test.folder.insert_view(view, None, uid1.as_i64());
  }

  // Add private views for each user
  folder_test
    .folder
    .add_private_view_ids(vec![user1_private_doc.clone()], uid1.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![user2_private_doc.clone()], uid2.as_i64());

  // Export folder data for user1
  let folder_data_user1 = folder_test
    .folder
    .get_folder_data(&workspace_id, uid1.as_i64())
    .unwrap();
  let hierarchy_user1 = FolderHierarchy::from_folder_data(folder_data_user1).unwrap();

  // Export folder data for user2
  let folder_data_user2 = folder_test
    .folder
    .get_folder_data(&workspace_id, uid2.as_i64())
    .unwrap();
  let hierarchy_user2 = FolderHierarchy::from_folder_data(folder_data_user2).unwrap();

  // Test that user1 sees only their private view
  assert!(hierarchy_user1.is_in_section(&user1_private_doc, FolderSection::Private));
  assert!(!hierarchy_user1.is_in_section(&user2_private_doc, FolderSection::Private));
  assert!(!hierarchy_user1.is_in_section(&shared_doc, FolderSection::Private));

  // Test that user2 sees only their private view
  assert!(!hierarchy_user2.is_in_section(&user1_private_doc, FolderSection::Private));
  assert!(hierarchy_user2.is_in_section(&user2_private_doc, FolderSection::Private));
  assert!(!hierarchy_user2.is_in_section(&shared_doc, FolderSection::Private));

  // Check private section views for each user
  let user1_private_views = hierarchy_user1.get_section_views(FolderSection::Private);
  assert_eq!(user1_private_views.len(), 1);
  assert_eq!(user1_private_views[0].name, "User1 Private Doc");

  let user2_private_views = hierarchy_user2.get_section_views(FolderSection::Private);
  assert_eq!(user2_private_views.len(), 1);
  assert_eq!(user2_private_views[0].name, "User2 Private Doc");
}

#[test]
fn test_folder_hierarchy_private_section_nested_views() {
  let uid = UserId::from(2001);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  // Create a private folder with children
  let private_folder_id = Uuid::new_v4().to_string();
  let child1_id = Uuid::new_v4().to_string();
  let child2_id = Uuid::new_v4().to_string();
  let nested_child_id = Uuid::new_v4().to_string();

  let private_folder = View {
    id: private_folder_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Private Folder".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(child1_id.clone()),
      ViewIdentifier::new(child2_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: Some(ViewIcon {
      ty: IconType::Emoji,
      value: "🔒".to_string(),
    }),
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let child1 = View {
    id: child1_id.clone(),
    parent_view_id: private_folder_id.clone(),
    name: "Private Child 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(nested_child_id.clone())]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let child2 = View {
    id: child2_id.clone(),
    parent_view_id: private_folder_id.clone(),
    name: "Private Child 2".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Grid,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let nested_child = View {
    id: nested_child_id.clone(),
    parent_view_id: child1_id.clone(),
    name: "Nested Private Child".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views
  folder_test
    .folder
    .insert_view(private_folder, None, uid.as_i64());
  folder_test.folder.insert_view(child1, None, uid.as_i64());
  folder_test.folder.insert_view(child2, None, uid.as_i64());
  folder_test
    .folder
    .insert_view(nested_child, None, uid.as_i64());

  // Add only the parent folder to private section
  folder_test
    .folder
    .add_private_view_ids(vec![private_folder_id.clone()], uid.as_i64());

  // Export and create hierarchy
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test that only the parent is in private section, not its children
  assert!(hierarchy.is_in_section(&private_folder_id, FolderSection::Private));
  assert!(!hierarchy.is_in_section(&child1_id, FolderSection::Private));
  assert!(!hierarchy.is_in_section(&child2_id, FolderSection::Private));
  assert!(!hierarchy.is_in_section(&nested_child_id, FolderSection::Private));

  // Test private section views
  let private_views = hierarchy.get_section_views(FolderSection::Private);
  assert_eq!(private_views.len(), 1);
  assert_eq!(private_views[0].name, "Private Folder");

  let private_folder_uuid = Uuid::parse_str(&private_folder_id).unwrap();
  assert_eq!(hierarchy.get_depth(&private_folder_uuid), 1); // Direct child of workspace

  // Test that we can still navigate to children of private views
  let private_folder_children = hierarchy.get_children(&private_folder_uuid).unwrap();
  assert_eq!(private_folder_children.len(), 2);

  // Test descendants of private folder
  let descendants = hierarchy.get_descendants(&private_folder_uuid);
  assert_eq!(descendants.len(), 3); // child1, child2, nested_child
}

#[test]
fn test_folder_hierarchy_private_section_mixed_ownership() {
  let uid1 = UserId::from(3001);
  let uid2 = UserId::from(3002);
  let uid3 = UserId::from(3003);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid1.clone(), &workspace_id);

  // Create views with mixed ownership
  let doc1_id = Uuid::new_v4().to_string();
  let doc2_id = Uuid::new_v4().to_string();
  let doc3_id = Uuid::new_v4().to_string();
  let doc4_id = Uuid::new_v4().to_string();

  // User1's document
  folder_test.folder.insert_view(
    View {
      id: doc1_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Doc1 by User1".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid1.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(uid1.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    uid1.as_i64(),
  );

  // User2's document
  folder_test.folder.insert_view(
    View {
      id: doc2_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Doc2 by User2".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid2.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(uid2.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    uid2.as_i64(),
  );

  // Document created by User1 but edited by User2
  folder_test.folder.insert_view(
    View {
      id: doc3_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Doc3 created by User1, edited by User2".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid1.as_i64()),
      last_edited_time: 100,
      last_edited_by: Some(uid2.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    uid1.as_i64(),
  );

  // User3's document
  folder_test.folder.insert_view(
    View {
      id: doc4_id.clone(),
      parent_view_id: workspace_id.clone(),
      name: "Doc4 by User3".to_string(),
      children: RepeatedViewIdentifier::new(vec![]),
      created_at: 0,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon: None,
      created_by: Some(uid3.as_i64()),
      last_edited_time: 0,
      last_edited_by: Some(uid3.as_i64()),
      is_locked: None,
      extra: None,
    },
    None,
    uid3.as_i64(),
  );

  // Each user marks different documents as private
  folder_test
    .folder
    .add_private_view_ids(vec![doc1_id.clone(), doc3_id.clone()], uid1.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![doc2_id.clone(), doc3_id.clone()], uid2.as_i64());
  folder_test
    .folder
    .add_private_view_ids(vec![doc4_id.clone()], uid3.as_i64());

  // Test User1's view
  let folder_data_user1 = folder_test
    .folder
    .get_folder_data(&workspace_id, uid1.as_i64())
    .unwrap();
  let hierarchy_user1 = FolderHierarchy::from_folder_data(folder_data_user1).unwrap();

  let user1_private = hierarchy_user1.get_section_views(FolderSection::Private);
  assert_eq!(user1_private.len(), 2);
  let private_names: Vec<&str> = user1_private.iter().map(|v| v.name.as_str()).collect();
  assert!(private_names.contains(&"Doc1 by User1"));
  assert!(private_names.contains(&"Doc3 created by User1, edited by User2"));

  // Test User2's view
  let folder_data_user2 = folder_test
    .folder
    .get_folder_data(&workspace_id, uid2.as_i64())
    .unwrap();
  let hierarchy_user2 = FolderHierarchy::from_folder_data(folder_data_user2).unwrap();

  let user2_private = hierarchy_user2.get_section_views(FolderSection::Private);
  assert_eq!(user2_private.len(), 2);
  let private_names: Vec<&str> = user2_private.iter().map(|v| v.name.as_str()).collect();
  assert!(private_names.contains(&"Doc2 by User2"));
  assert!(private_names.contains(&"Doc3 created by User1, edited by User2"));

  // Test User3's view
  let folder_data_user3 = folder_test
    .folder
    .get_folder_data(&workspace_id, uid3.as_i64())
    .unwrap();
  let hierarchy_user3 = FolderHierarchy::from_folder_data(folder_data_user3).unwrap();

  let user3_private = hierarchy_user3.get_section_views(FolderSection::Private);
  assert_eq!(user3_private.len(), 1);
  assert_eq!(user3_private[0].name, "Doc4 by User3");

  // Verify that doc3 is private for both user1 and user2
  assert!(hierarchy_user1.is_in_section(&doc3_id, FolderSection::Private));
  assert!(hierarchy_user2.is_in_section(&doc3_id, FolderSection::Private));
  assert!(!hierarchy_user3.is_in_section(&doc3_id, FolderSection::Private));
}

#[test]
fn test_folder_hierarchy_private_section_empty() {
  let uid = UserId::from(4001);
  let workspace_id = Uuid::new_v4().to_string();

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  // Create some views but don't mark any as private
  let doc1_id = Uuid::new_v4().to_string();
  let doc2_id = Uuid::new_v4().to_string();

  folder_test.folder.insert_view(
    make_test_view(&doc1_id, &workspace_id, vec![]),
    None,
    uid.as_i64(),
  );

  folder_test.folder.insert_view(
    make_test_view(&doc2_id, &workspace_id, vec![]),
    None,
    uid.as_i64(),
  );

  // Export without adding any private views
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  // Test that private section is empty
  let private_views = hierarchy.get_section_views(FolderSection::Private);
  assert_eq!(private_views.len(), 0);

  // Verify no views are in private section
  assert!(!hierarchy.is_in_section(&doc1_id, FolderSection::Private));
  assert!(!hierarchy.is_in_section(&doc2_id, FolderSection::Private));
}

#[test]
fn test_folder_hierarchy_descendants_with_depth_limit() {
  let uid = UserId::from(5001);
  let workspace_id = Uuid::new_v4().to_string();

  // Create a deeper hierarchy:
  // workspace
  //   └── folder1
  //       ├── doc1
  //       └── subfolder
  //           └── subdoc

  let mut folder_test = create_folder_with_workspace(uid.clone(), &workspace_id);

  let folder1_id = Uuid::new_v4().to_string();
  let doc1_id = Uuid::new_v4().to_string();
  let subfolder_id = Uuid::new_v4().to_string();
  let subdoc_id = Uuid::new_v4().to_string();

  let folder1 = View {
    id: folder1_id.clone(),
    parent_view_id: workspace_id.clone(),
    name: "Folder 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![
      ViewIdentifier::new(doc1_id.clone()),
      ViewIdentifier::new(subfolder_id.clone()),
    ]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let doc1 = View {
    id: doc1_id.clone(),
    parent_view_id: folder1_id.clone(),
    name: "Document 1".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let subfolder = View {
    id: subfolder_id.clone(),
    parent_view_id: folder1_id.clone(),
    name: "Subfolder".to_string(),
    children: RepeatedViewIdentifier::new(vec![ViewIdentifier::new(subdoc_id.clone())]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  let subdoc = View {
    id: subdoc_id.clone(),
    parent_view_id: subfolder_id.clone(),
    name: "Sub Document".to_string(),
    children: RepeatedViewIdentifier::new(vec![]),
    created_at: 0,
    is_favorite: false,
    layout: ViewLayout::Document,
    icon: None,
    created_by: Some(uid.as_i64()),
    last_edited_time: 0,
    last_edited_by: None,
    is_locked: None,
    extra: None,
  };

  // Insert views
  folder_test.folder.insert_view(folder1, None, uid.as_i64());
  folder_test.folder.insert_view(doc1, None, uid.as_i64());
  folder_test
    .folder
    .insert_view(subfolder, None, uid.as_i64());
  folder_test.folder.insert_view(subdoc, None, uid.as_i64());

  // Export and create hierarchy
  let folder_data = folder_test
    .folder
    .get_folder_data(&workspace_id, uid.as_i64())
    .unwrap();
  let hierarchy = FolderHierarchy::from_folder_data(folder_data).unwrap();

  let folder1_uuid = Uuid::parse_str(&folder1_id).unwrap();

  // Test depth = 0: no descendants
  let descendants_d0 = hierarchy.get_descendants_with_children(&folder1_uuid, 0);
  assert_eq!(descendants_d0.len(), 0);

  // Test depth = 1: only direct children (doc1 and subfolder)
  let descendants_d1 = hierarchy.get_descendants_with_children(&folder1_uuid, 1);
  assert_eq!(descendants_d1.len(), 2);
  let names_d1: Vec<String> = descendants_d1.iter().map(|d| d.view.name.clone()).collect();
  assert!(names_d1.contains(&"Document 1".to_string()));
  assert!(names_d1.contains(&"Subfolder".to_string()));

  // Test depth = 2: children and grandchildren (doc1, subfolder, subdoc)
  let descendants_d2 = hierarchy.get_descendants_with_children(&folder1_uuid, 2);
  assert_eq!(descendants_d2.len(), 3);

  // Find each descendant and verify its properties
  let doc1_desc = descendants_d2
    .iter()
    .find(|d| d.view.name == "Document 1")
    .unwrap();
  assert_eq!(doc1_desc.children.len(), 0); // No children

  let subfolder_desc = descendants_d2
    .iter()
    .find(|d| d.view.name == "Subfolder")
    .unwrap();
  assert_eq!(subfolder_desc.children.len(), 1); // Has subdoc as child

  let subdoc_desc = descendants_d2
    .iter()
    .find(|d| d.view.name == "Sub Document")
    .unwrap();
  assert_eq!(subdoc_desc.children.len(), 0); // No children

  // Test from workspace with different depths
  let workspace_uuid = Uuid::parse_str(&workspace_id).unwrap();

  // depth = 1: only folder1
  let ws_descendants_d1 = hierarchy.get_descendants_with_children(&workspace_uuid, 1);
  assert_eq!(ws_descendants_d1.len(), 1);
  assert_eq!(ws_descendants_d1[0].view.name, "Folder 1");

  // depth = 2: folder1, doc1, subfolder
  let ws_descendants_d2 = hierarchy.get_descendants_with_children(&workspace_uuid, 2);
  assert_eq!(ws_descendants_d2.len(), 3);

  // depth = 3: all views
  let ws_descendants_d3 = hierarchy.get_descendants_with_children(&workspace_uuid, 3);
  assert_eq!(ws_descendants_d3.len(), 4);
}
