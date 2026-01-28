use collab::document::blocks::*;
use collab::preclude::*;

#[test]
fn test_create_document_with_all_mention_types() {
  let doc_id = "mention_integration_test";
  let test = crate::util::DocumentTest::new(1, doc_id);
  let mut document = test.document;
  let page_id = document.get_page_id().unwrap();

  // Create a paragraph with mixed content and all mention types
  let block_id = nanoid::nanoid!(6);
  let text_id = nanoid::nanoid!(6);

  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_owned(),
    parent: page_id.clone(),
    children: "".to_string(),
    external_id: Some(text_id.clone()),
    external_type: Some("text".to_owned()),
    data: Default::default(),
  };

  document.insert_block(block, None).unwrap();

  // Build a rich delta with all mention types
  let deltas = vec![
    // Regular text
    TextDelta::Inserted("Hello ".to_string(), None),
    // Person mention
    build_mention_person_delta("person_123", "Alice", &page_id, Some(&block_id), None),
    // More text
    TextDelta::Inserted("! Check ".to_string(), None),
    // Page mention
    build_mention_page_delta(MentionPageType::Page, "page_456", Some("block_789"), None),
    // Text
    TextDelta::Inserted(" and ".to_string(), None),
    // Child page mention
    build_mention_page_delta(MentionPageType::ChildPage, "child_page_999", None, None),
    // Text
    TextDelta::Inserted(" on ".to_string(), None),
    // Date mention with reminder
    build_mention_date_delta(
      "2025-01-30T10:00:00Z",
      Some("reminder_abc"),
      Some("atTimeOfEvent"),
      true,
    ),
    // Text
    TextDelta::Inserted(". Link: ".to_string(), None),
    // External link mention
    build_mention_external_link_delta("https://appflowy.io"),
  ];

  // Apply deltas to document
  let delta_json = serde_json::to_string(&deltas).unwrap();
  document.apply_text_delta(&text_id, delta_json);

  // Read back and verify all mentions
  let retrieved_deltas = document.get_block_delta(&block_id).unwrap().1;

  // Count mentions
  let mut person_count = 0;
  let mut page_count = 0;
  let mut child_page_count = 0;
  let mut date_count = 0;
  let mut link_count = 0;

  for delta in &retrieved_deltas {
    match extract_mention_data(delta) {
      Some(MentionData::Person {
        person_id,
        person_name,
        page_id: pid,
        block_id: bid,
        ..
      }) => {
        person_count += 1;
        assert_eq!(person_id, "person_123");
        assert_eq!(person_name, "Alice");
        assert_eq!(pid, page_id);
        assert_eq!(bid, Some(block_id.clone()));
      },
      Some(MentionData::Page {
        page_id: pid,
        block_id: bid,
        ..
      }) => {
        page_count += 1;
        assert_eq!(pid, "page_456");
        assert_eq!(bid, Some("block_789".to_string()));
      },
      Some(MentionData::ChildPage { page_id: pid }) => {
        child_page_count += 1;
        assert_eq!(pid, "child_page_999");
      },
      Some(MentionData::Date {
        date,
        include_time,
        reminder_id,
        reminder_option,
      }) => {
        date_count += 1;
        assert_eq!(date, "2025-01-30T10:00:00Z");
        assert!(include_time);
        assert_eq!(reminder_id, Some("reminder_abc".to_string()));
        assert_eq!(reminder_option, Some("atTimeOfEvent".to_string()));
      },
      Some(MentionData::ExternalLink { url }) => {
        link_count += 1;
        assert_eq!(url, "https://appflowy.io");
      },
      None => {
        // Regular text, skip
      },
    }
  }

  assert_eq!(person_count, 1, "Should have 1 person mention");
  assert_eq!(page_count, 1, "Should have 1 page mention");
  assert_eq!(child_page_count, 1, "Should have 1 child page mention");
  assert_eq!(date_count, 1, "Should have 1 date mention");
  assert_eq!(link_count, 1, "Should have 1 external link mention");
}

#[test]
fn test_mention_extraction_functions() {
  // Test person mention extraction
  let person_delta = build_mention_person_delta("user1", "Bob", "doc1", None, Some("row1"));
  assert!(is_mention(&person_delta));
  assert_eq!(
    extract_mention_type(&person_delta),
    Some("person".to_string())
  );
  assert_eq!(extract_person_id(&person_delta), Some("user1".to_string()));

  // Test page mention extraction
  let page_delta = build_mention_page_delta(MentionPageType::Page, "page1", None, None);
  assert!(is_mention(&page_delta));
  assert_eq!(extract_mention_type(&page_delta), Some("page".to_string()));
  assert_eq!(extract_page_id(&page_delta), Some("page1".to_string()));

  // Test date mention extraction
  let date_delta = build_mention_date_delta("2025-02-01T00:00:00Z", None, None, false);
  assert!(is_mention(&date_delta));
  assert_eq!(extract_mention_type(&date_delta), Some("date".to_string()));
  assert_eq!(
    extract_date(&date_delta),
    Some("2025-02-01T00:00:00Z".to_string())
  );

  // Test external link extraction
  let link_delta = build_mention_external_link_delta("https://rust-lang.org");
  assert!(is_mention(&link_delta));
  assert_eq!(
    extract_mention_type(&link_delta),
    Some("externalLink".to_string())
  );
  assert_eq!(
    extract_url(&link_delta),
    Some("https://rust-lang.org".to_string())
  );

  // Test non-mention delta
  let regular_delta = TextDelta::Inserted("Not a mention".to_string(), None);
  assert!(!is_mention(&regular_delta));
  assert_eq!(extract_mention_type(&regular_delta), None);
}

#[test]
fn test_flutter_compatibility() {
  // This test verifies that the Rust helpers produce data structures
  // identical to Flutter's buildMentionXXXAttributes functions

  // Person mention - Flutter: MentionBlockKeys.buildMentionPersonAttributes
  let person_delta = build_mention_person_delta(
    "person_id_123",
    "John Smith",
    "page_id_456",
    Some("block_id_789"),
    Some("row_id_abc"),
  );

  if let TextDelta::Inserted(text, Some(attrs)) = &person_delta {
    assert_eq!(text, "$", "Should use $ as mention character");

    // Verify mention structure
    if let Some(Any::Map(mention)) = attrs.get("mention") {
      assert_eq!(mention.get("type").unwrap().to_string(), "person");
      assert_eq!(
        mention.get("person_id").unwrap().to_string(),
        "person_id_123"
      );
      assert_eq!(
        mention.get("person_name").unwrap().to_string(),
        "John Smith"
      );
      assert_eq!(mention.get("page_id").unwrap().to_string(), "page_id_456");
      assert_eq!(mention.get("block_id").unwrap().to_string(), "block_id_789");
      assert_eq!(mention.get("row_id").unwrap().to_string(), "row_id_abc");
    } else {
      panic!("Expected mention map in attributes");
    }
  } else {
    panic!("Expected Inserted delta with attributes");
  }

  // Date mention - Flutter: MentionBlockKeys.buildMentionDateAttributes
  let date_delta = build_mention_date_delta(
    "2025-01-30T12:00:00.000Z",
    Some("reminder_xyz"),
    Some("atTimeOfEvent"),
    true,
  );

  if let TextDelta::Inserted(text, Some(attrs)) = &date_delta {
    assert_eq!(text, "$");

    if let Some(Any::Map(mention)) = attrs.get("mention") {
      assert_eq!(mention.get("type").unwrap().to_string(), "date");
      assert_eq!(
        mention.get("date").unwrap().to_string(),
        "2025-01-30T12:00:00.000Z"
      );
      assert_eq!(mention.get("include_time").unwrap().to_string(), "true");
      assert_eq!(
        mention.get("reminder_id").unwrap().to_string(),
        "reminder_xyz"
      );
      assert_eq!(
        mention.get("reminder_option").unwrap().to_string(),
        "atTimeOfEvent"
      );
    } else {
      panic!("Expected mention map in attributes");
    }
  } else {
    panic!("Expected Inserted delta with attributes");
  }
}

#[test]
fn test_mention_data_enum() {
  // Test extracting complete mention data using the enum

  let person = build_mention_person_delta("p1", "Alice", "d1", None, None);
  match extract_mention_data(&person) {
    Some(MentionData::Person {
      person_id,
      person_name,
      ..
    }) => {
      assert_eq!(person_id, "p1");
      assert_eq!(person_name, "Alice");
    },
    _ => panic!("Expected Person mention data"),
  }

  let page = build_mention_page_delta(MentionPageType::Page, "p2", Some("b1"), None);
  match extract_mention_data(&page) {
    Some(MentionData::Page {
      page_id, block_id, ..
    }) => {
      assert_eq!(page_id, "p2");
      assert_eq!(block_id, Some("b1".to_string()));
    },
    _ => panic!("Expected Page mention data"),
  }

  let child = build_mention_page_delta(MentionPageType::ChildPage, "p3", None, None);
  match extract_mention_data(&child) {
    Some(MentionData::ChildPage { page_id }) => {
      assert_eq!(page_id, "p3");
    },
    _ => panic!("Expected ChildPage mention data"),
  }

  let date = build_mention_date_delta("2025-01-30T10:00:00Z", None, None, false);
  match extract_mention_data(&date) {
    Some(MentionData::Date {
      date, include_time, ..
    }) => {
      assert_eq!(date, "2025-01-30T10:00:00Z");
      assert!(!include_time);
    },
    _ => panic!("Expected Date mention data"),
  }

  let link = build_mention_external_link_delta("https://appflowy.io");
  match extract_mention_data(&link) {
    Some(MentionData::ExternalLink { url }) => {
      assert_eq!(url, "https://appflowy.io");
    },
    _ => panic!("Expected ExternalLink mention data"),
  }
}

#[test]
fn test_date_without_time() {
  // Flutter often sends dates without time
  let date_delta = build_mention_date_delta("2025-01-30T00:00:00Z", None, None, false);

  match extract_mention_data(&date_delta) {
    Some(MentionData::Date {
      date,
      include_time,
      reminder_id,
      reminder_option,
    }) => {
      assert_eq!(date, "2025-01-30T00:00:00Z");
      assert!(!include_time);
      assert_eq!(reminder_id, None);
      assert_eq!(reminder_option, None);
    },
    _ => panic!("Expected Date mention data"),
  }
}

#[test]
fn test_page_with_block_deep_link() {
  // Test page mention with block ID for deep linking
  let page_delta = build_mention_page_delta(
    MentionPageType::Page,
    "target_page",
    Some("target_block"),
    None,
  );

  match extract_mention_data(&page_delta) {
    Some(MentionData::Page {
      page_id,
      block_id,
      row_id,
    }) => {
      assert_eq!(page_id, "target_page");
      assert_eq!(block_id, Some("target_block".to_string()));
      assert_eq!(row_id, None);
    },
    _ => panic!("Expected Page mention data with block_id"),
  }
}
