use crate::core::{Belongings, BelongingsArray};
use collab::preclude::{Map, MapRefWrapper, ReadTxn, TransactionMut};
use serde::{Deserialize, Serialize};
use serde_repr::*;

const VIEW_ID: &str = "id";
const VIEW_NAME: &str = "name";
const VIEW_BID: &str = "bid";
const VIEW_DESC: &str = "desc";
const VIEW_LAYOUT: &str = "layout";
const VIEW_CREATE_AT: &str = "created_at";
const VIEW_BELONGINGS: &str = "belongings";

pub struct ViewsMap {
    container: MapRefWrapper,
}

impl ViewsMap {
    pub fn new(root: MapRefWrapper) -> ViewsMap {
        Self { container: root }
    }

    pub fn get_view(&self, view_id: &str) -> Option<View> {
        let txn = self.container.transact();
        self.get_view_with_txn(&txn, view_id)
    }

    pub fn get_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Option<View> {
        let map_ref = self.container.get_map_with_txn(txn, view_id)?;
        let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
        let name = map_ref.get_str_with_txn(txn, VIEW_NAME).unwrap_or_default();
        let bid = map_ref.get_str_with_txn(txn, VIEW_BID);
        let desc = map_ref.get_str_with_txn(txn, VIEW_DESC).unwrap_or_default();
        let created_at = map_ref
            .get_i64_with_txn(txn, VIEW_CREATE_AT)
            .unwrap_or_default();
        let layout = map_ref.get_i64_with_txn(txn, VIEW_LAYOUT)? as u8;
        let array = map_ref.get_array_ref(VIEW_BELONGINGS)?;
        let belongings = BelongingsArray::from_array(array).get_belongings();
        Some(View {
            id,
            bid,
            name,
            desc,
            belongings,
            created_at,
            layout,
        })
    }

    pub fn insert_view(&self, view: View) {
        self.container
            .with_transact_mut(|txn| self.insert_view_with_txn(txn, view));
    }

    pub fn insert_view_with_txn(&self, txn: &mut TransactionMut, view: View) {
        let map_ref = self.container.insert_map_with_txn(txn, &view.id);
        ViewUpdateBuilder::new(txn, map_ref)
            .with_id(view.id)
            .with_name(view.name)
            .with_bid(view.bid)
            .with_desc(view.desc)
            .with_layout(view.layout)
            .with_created_at(view.created_at)
            .with_belongings(view.belongings)
            .done();
    }

    pub fn delete_view(&self, view_id: &str) {
        self.container
            .with_transact_mut(|txn| self.delete_view_with_txn(txn, view_id));
    }

    pub fn delete_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
        self.container.remove(txn, view_id);
    }

    pub fn update_view<F>(&self, view_id: &str, f: F)
    where
        F: FnOnce(ViewUpdateBuilder),
    {
        self.container.with_transact_mut(|txn| {
            let map_ref = self.container.insert_map_with_txn(txn, view_id);
            let builder = ViewUpdateBuilder::new(txn, map_ref);
            f(builder);
        })
    }
}

pub struct ViewUpdateBuilder<'a, 'b> {
    map_ref: MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewUpdateBuilder<'a, 'b> {
    pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
        Self { map_ref, txn }
    }

    pub fn with_id<T: AsRef<str>>(self, id: T) -> Self {
        self.map_ref.insert_with_txn(self.txn, VIEW_ID, id.as_ref());
        self
    }

    pub fn with_name<T: AsRef<str>>(self, name: T) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_NAME, name.as_ref());
        self
    }

    pub fn with_bid(self, bid: Option<String>) -> Self {
        self.map_ref.insert_with_txn(self.txn, VIEW_BID, bid);
        self
    }

    pub fn with_desc<T: AsRef<str>>(self, desc: T) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_DESC, desc.as_ref());
        self
    }

    pub fn with_layout(self, layout: u8) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_LAYOUT, layout as i64);
        self
    }

    pub fn with_created_at(self, created_at: i64) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_CREATE_AT, created_at);
        self
    }

    pub fn with_belongings(self, belongings: Belongings) -> Self {
        self.map_ref
            .insert_array_with_txn(self.txn, VIEW_BELONGINGS, belongings.into_inner());
        self
    }

    pub fn done(self) {}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct View {
    pub id: String,
    // bid short for belong to id
    pub bid: Option<String>,
    pub name: String,
    pub desc: String,
    pub belongings: Belongings,
    pub created_at: i64,
    pub layout: u8,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ViewLayout {
    Document = 0,
    Grid = 1,
    Board = 2,
    Calendar = 3,
}
