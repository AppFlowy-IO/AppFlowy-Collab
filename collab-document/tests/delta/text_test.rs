use crate::util::create_document;
use collab_document::blocks::{Block, BlockBuilder, TextAction, TextData};

#[test]
fn update_text_test() {
    let doc_id = "1";
    let test = create_document(doc_id);

    // Text
    let text_id = "fake_text_id";
    let text_data = TextData {
        text_id: text_id.to_string(),
    };
    let block = Block {
        id: "1".to_string(),
        ty: "text".to_string(),
        next: "".to_string(),
        first_child: "".to_string(),
        data: text_data.to_string(),
    };
    test.document.blocks.insert_block(block);

    // Update text
    let texts = &test.document.texts;
    texts.edit_text(
        text_id,
        vec![
            TextAction::Push {
                s: "abc".to_string(),
            },
            TextAction::Insert {
                index: 0,
                s: "123".to_string(),
                attrs: None,
            },
        ],
    );
    assert_eq!(texts.get_str(text_id).unwrap(), "123abc");
}

#[test]
fn edit_text_multiple_time_test() {
    let doc_id = "1";
    let test = create_document(doc_id);

    // Text
    let text_id = "fake_text_id";
    let text_data = TextData {
        text_id: text_id.to_string(),
    };

    // Create block
    test.document.blocks.create_block("1", |builder| {
        builder.with_type("text").with_data(text_data).build()
    });

    // Update text
    let texts = &test.document.texts;
    texts.edit_text(
        text_id,
        vec![TextAction::Push {
            s: "abc".to_string(),
        }],
    );
    texts.edit_text(
        text_id,
        vec![
            TextAction::Push {
                s: "123".to_string(),
            },
            TextAction::Remove { index: 0, len: 2 },
        ],
    );
    assert_eq!(texts.get_str(text_id).unwrap(), "c123");
    // let deltas = texts.get_delta(text_id);
    // println!("{:?}", deltas);
}
