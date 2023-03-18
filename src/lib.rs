mod collaborator;
mod error;
mod util;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use yrs::types::text::YChange;
    use yrs::updates::decoder::Decode;
    use yrs::*;

    #[test]
    fn test1() {
        let doc = Doc::new();
        let mut text = doc.get_or_insert_text("name");
        let mut txn = doc.transact_mut();
        text.push(&mut txn, "hello");
        text.push(&mut txn, " world");
        drop(txn);

        let restore_text = doc.get_or_insert_text("name");
        let txn = doc.transact();
        let s = restore_text.get_string(&txn);

        assert_eq!(s, "hello world")
    }

    #[test]
    fn test2() {
        let doc = Doc::new();
        let mut text = doc.get_or_insert_text("name");
        let mut txn = doc.transact_mut();
        let state = txn.state_vector();
        text.push(&mut txn, "hello");
        text.push(&mut txn, " world");
        let bytes = txn.encode_state_as_update_v2(&state);
        drop(txn);

        let remote_doc_1 = Doc::new();
        let mut remote_text_1 = remote_doc_1.get_or_insert_text("name");
        let mut txn_1 = remote_doc_1.transact_mut();
        let state_1 = txn_1.state_vector();
        remote_text_1.push(&mut txn_1, "123");
        let bytes_1 = txn_1.encode_state_as_update_v2(&state_1);

        txn_1.apply_update(Update::decode_v2(&bytes).unwrap());
        drop(txn_1);

        let remote_doc_2 = Doc::new();
        let mut remote_text_2 = remote_doc_2.get_or_insert_text("name");
        let mut txn_2 = remote_text_2.transact_mut();
        remote_text_2.push(&mut txn_2, "abc");
        txn_2.apply_update(Update::decode_v2(&bytes).unwrap());
        txn_2.apply_update(Update::decode_v2(&bytes_1).unwrap());
        drop(txn_2);

        let text = text.get_string(&doc.transact());
        let text_1 = remote_text_1.get_string(&remote_doc_1.transact());
        let text_2 = remote_text_2.get_string(&remote_doc_2.transact());

        println!("{}", text);
        println!("{}", text_1);
        println!("{}", text_2);
    }

    #[test]
    fn it_works() {
        let doc = Doc::new();
        let mut text = doc.get_or_insert_text("name");
        let subscription = text.observe(|transaction, event| {
            println!("local: {:?}", event.delta(transaction));
        });
        // every operation in Yrs happens in scope of a transaction
        let mut txn = doc.transact_mut();
        // append text to our collaborative document
        text.push(&mut txn, "Hello from yrs!");

        // simulate update with remote peer
        let remote_doc = Doc::new();
        let mut remote_text = remote_doc.get_or_insert_text("name");
        let subscription = remote_text.observe(|transaction, event| {
            println!("remote_text: {:?}", event.delta(transaction));
        });
        let mut remote_txn = remote_doc.transact_mut();

        // in order to exchange data with other documents
        // we first need to create a state vector
        let state_vector = remote_txn.state_vector();

        // now compute a differential update based on remote document's
        // state vector
        let bytes = txn.encode_diff_v1(&state_vector);

        // both update and state vector are serializable, we can pass them
        // over the wire now apply update to a remote document
        let update = Update::decode_v1(&bytes).unwrap();
        remote_txn.apply_update(update);

        // display raw text (no attributes)
        println!("{}", remote_text.get_string(&remote_txn));

        // create sequence of text chunks with optional format attributes
        let diff = remote_text.diff(&remote_txn, YChange::identity);

        println!("{:?}", diff);
    }
}
