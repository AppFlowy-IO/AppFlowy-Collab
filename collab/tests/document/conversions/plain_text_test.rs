use crate::util::DocumentTest;
use collab::document::{
  DefaultPlainTextResolver, PlainTextExportOptions, blocks::Block, document::Document,
};
use nanoid::nanoid;
use serde_json::json;
use std::collections::HashMap;

#[test]
fn plain_text_1_test() {
  let doc_id = "1";
  let test = DocumentTest::new(1, doc_id);
  let mut document = test.document;
  let paragraphs = vec![
    "Welcome to AppFlowy!".to_string(),
    "Here are the basics".to_string(),
    "Click anywhere and just start typing.".to_string(),
    "Highlight any text, and use the editing menu to _style_ **your** <u>writing</u> `however` you ~~like.~~".to_string(),
    "As soon as you type `/` a menu will pop up. Select different types of content blocks you can add.".to_string(),
    "Type `/` followed by `/bullet` or `/num` to create a list.".to_string(),
    "Click `+ New Page `button at the bottom of your sidebar to add a new page.".to_string(),
    "Click `+` next to any page title in the sidebar to quickly add a new subpage, `Document`, `Grid`, or `Kanban Board`.".to_string(),
  ];
  insert_paragraphs(&mut document, paragraphs.clone());

  let splitted = document.to_plain_text();
  // remove the empty lines at the beginning and the end
  // the first one and the last one are empty
  assert_eq!(splitted.len(), 8);

  for i in 0..splitted.len() {
    assert_eq!(splitted[i], paragraphs[i]);
  }
}

#[test]
fn plain_text_mentions_default_resolver() {
  let doc_id = "mention-default";
  let test = DocumentTest::new(1, doc_id);
  let mut document = test.document;
  let page_id = document.get_page_id().unwrap();
  let mut prev_id = "".to_string();

  insert_paragraph_with_delta(
    &mut document,
    &page_id,
    &mut prev_id,
    json!([
      {"insert": "Hello "},
      {"insert": "$", "attributes": {"mention": {
        "type": "person",
        "person_id": "person-123",
        "person_name": "Mock User",
        "page_id": page_id
      }}},
      {"insert": "!"}
    ])
    .to_string(),
  );

  insert_paragraph_with_delta(
    &mut document,
    &page_id,
    &mut prev_id,
    json!([
      {"insert": "Related "},
      {"insert": "$", "attributes": {"mention": {
        "type": "page",
        "page_id": "doc-123"
      }}}
    ])
    .to_string(),
  );

  insert_subpage_block(&mut document, &page_id, &mut prev_id, "doc-embedded");

  let plain = document
    .to_plain_text()
    .into_iter()
    .filter(|line| !line.trim().is_empty())
    .collect::<Vec<_>>();
  assert_eq!(
    plain,
    vec!["Hello @Mock User!", "Related [[doc-123]]", "doc-embedded"]
  );
}

#[test]
fn plain_text_mentions_custom_resolver() {
  let doc_id = "mention-custom";
  let test = DocumentTest::new(1, doc_id);
  let mut document = test.document;
  let page_id = document.get_page_id().unwrap();
  let mut prev_id = "".to_string();

  insert_paragraph_with_delta(
    &mut document,
    &page_id,
    &mut prev_id,
    json!([
      {"insert": "Hello "},
      {"insert": "$", "attributes": {"mention": {
        "type": "person",
        "person_id": "person-123",
        "person_name": "Mock User",
        "page_id": page_id
      }}},
      {"insert": "!"}
    ])
    .to_string(),
  );

  insert_paragraph_with_delta(
    &mut document,
    &page_id,
    &mut prev_id,
    json!([
      {"insert": "Related "},
      {"insert": "$", "attributes": {"mention": {
        "type": "page",
        "page_id": "doc-123"
      }}}
    ])
    .to_string(),
  );

  insert_subpage_block(&mut document, &page_id, &mut prev_id, "doc-embedded");

  let mut person_map = HashMap::new();
  person_map.insert("person-123".to_string(), "Alice".to_string());

  let mut doc_map = HashMap::new();
  doc_map.insert("doc-123".to_string(), "Design Doc".to_string());
  doc_map.insert("doc-embedded".to_string(), "Embedded Doc".to_string());

  let resolver = DefaultPlainTextResolver::new()
    .with_person_names(person_map)
    .with_document_titles(doc_map);
  let options = PlainTextExportOptions::from_default_resolver(resolver);

  let plain = document
    .to_plain_text_with_options(&options)
    .into_iter()
    .filter(|line| !line.trim().is_empty())
    .collect::<Vec<_>>();
  assert_eq!(
    plain,
    vec!["Hello @Alice!", "Related [[Design Doc]]", "Embedded Doc"]
  );
}

fn insert_paragraphs(document: &mut Document, paragraphs: Vec<String>) {
  let page_id = document.get_page_id().unwrap();
  let mut prev_id = "".to_string();
  for paragraph in paragraphs {
    let block_id = nanoid!(6);
    let text_id = nanoid!(6);
    let block = Block {
      id: block_id.clone(),
      ty: "paragraph".to_owned(),
      parent: page_id.clone(),
      children: "".to_string(),
      external_id: Some(text_id.clone()),
      external_type: Some("text".to_owned()),
      data: Default::default(),
    };

    document.insert_block(block, Some(prev_id.clone())).unwrap();
    prev_id.clone_from(&block_id);

    document.apply_text_delta(&text_id, format!(r#"[{{"insert": "{}"}}]"#, paragraph));
  }
}

fn insert_paragraph_with_delta(
  document: &mut Document,
  page_id: &str,
  prev_id: &mut String,
  delta_json: String,
) {
  let block_id = nanoid!(6);
  let text_id = nanoid!(6);
  let block = Block {
    id: block_id.clone(),
    ty: "paragraph".to_owned(),
    parent: page_id.to_string(),
    children: "".to_string(),
    external_id: Some(text_id.clone()),
    external_type: Some("text".to_owned()),
    data: Default::default(),
  };

  let insert_prev = if prev_id.is_empty() {
    None
  } else {
    Some(prev_id.clone())
  };
  document.insert_block(block, insert_prev).unwrap();
  prev_id.clone_from(&block_id);
  document.apply_text_delta(&text_id, delta_json);
}

fn insert_subpage_block(
  document: &mut Document,
  page_id: &str,
  prev_id: &mut String,
  view_id: &str,
) {
  let block_id = nanoid!(6);
  let mut data = HashMap::new();
  data.insert("viewId".to_string(), json!(view_id));

  let block = Block {
    id: block_id.clone(),
    ty: "sub_page".to_owned(),
    parent: page_id.to_string(),
    children: "".to_string(),
    external_id: None,
    external_type: None,
    data,
  };

  let insert_prev = if prev_id.is_empty() {
    None
  } else {
    Some(prev_id.clone())
  };
  document.insert_block(block, insert_prev).unwrap();
  prev_id.clone_from(&block_id);
}
