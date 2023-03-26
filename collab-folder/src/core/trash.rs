use crate::core::{TrashInfo, ViewsMap};
use collab::preclude::array::ArrayEvent;
use collab::preclude::{
    lib0Any, Array, ArrayRefWrapper, Change, Observable, ReadTxn, Subscription, TransactionMut,
    YrsValue,
};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::broadcast;

pub type TrashChangeSender = broadcast::Sender<TrashChange>;
pub type TrashChangeReceiver = broadcast::Receiver<TrashChange>;
type ArraySubscription = Subscription<Arc<dyn Fn(&TransactionMut, &ArrayEvent)>>;

#[derive(Debug, Clone)]
pub enum TrashChange {
    DidCreateTrash { ids: Vec<String> },
    DidDeleteTrash { ids: Vec<String> },
}

pub struct TrashArray {
    container: ArrayRefWrapper,
    view_map: Rc<ViewsMap>,
    #[allow(dead_code)]
    tx: Option<TrashChangeSender>,
    #[allow(dead_code)]
    subscription: Option<ArraySubscription>,
}

impl TrashArray {
    pub fn new(
        mut root: ArrayRefWrapper,
        view_map: Rc<ViewsMap>,
        tx: Option<TrashChangeSender>,
    ) -> Self {
        let subscription = subscribe_change(&mut root, tx.clone());
        Self {
            container: root,
            view_map,
            tx,
            subscription,
        }
    }

    pub fn get_all_trash(&self) -> Vec<TrashInfo> {
        let txn = self.container.transact();
        let items = self.get_all_trash_with_txn(&txn);
        items
            .into_iter()
            .map(|item| {
                let name = self
                    .view_map
                    .get_view_name_with_txn(&txn, &item.id)
                    .unwrap_or_default();
                TrashInfo {
                    id: item.id,
                    name,
                    created_at: item.created_at,
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn get_all_trash_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<TrashItem> {
        let mut trash = vec![];
        for value in self.container.iter(txn) {
            if let YrsValue::Any(any) = value {
                trash.push(TrashItem::from(any));
            }
        }
        trash
    }

    pub fn delete_trash(&self, id: &str) {
        self.container.with_transact_mut(|txn| {
            self.delete_trash_with_txn(txn, id);
        })
    }

    pub fn delete_trash_with_txn(&self, txn: &mut TransactionMut, id: &str) {
        if let Some(pos) = self
            .get_all_trash_with_txn(txn)
            .into_iter()
            .position(|item| item.id == id)
        {
            self.container.remove_with_txn(txn, pos as u32);
        }
    }

    pub fn add_trash(&self, trash: TrashItem) {
        self.container.with_transact_mut(|txn| {
            self.container.push_with_txn(txn, trash);
        })
    }

    pub fn add_trash_with_txn(&self, txn: &mut TransactionMut, trash: TrashItem) {
        self.container.push_with_txn(txn, trash);
    }

    pub fn clear(&self) {
        self.container.with_transact_mut(|txn| {
            let len = self.container.iter(txn).count();
            self.container.remove_range(txn, 0, len as u32);
        });
    }
}

fn subscribe_change(
    array: &mut ArrayRefWrapper,
    tx: Option<TrashChangeSender>,
) -> Option<ArraySubscription> {
    return if tx.is_some() {
        Some(array.observe(|txn, event| {
            for change in event.delta(txn) {
                match change {
                    Change::Added(_values) => {}
                    Change::Removed(_value) => {}
                    Change::Retain(_) => {}
                }
            }
        }))
    } else {
        None
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrashItem {
    pub id: String,
    pub created_at: i64,
}

impl From<lib0Any> for TrashItem {
    fn from(any: lib0Any) -> Self {
        let mut json = String::new();
        any.to_json(&mut json);
        serde_json::from_str(&json).unwrap()
    }
}

impl From<TrashItem> for lib0Any {
    fn from(item: TrashItem) -> Self {
        let json = serde_json::to_string(&item).unwrap();
        lib0Any::from_json(&json).unwrap()
    }
}
