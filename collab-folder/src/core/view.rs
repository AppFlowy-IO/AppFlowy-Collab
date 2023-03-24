use crate::core::{Belongings, BelongingsArray};
use anyhow::{anyhow, bail, Result};
use collab::core::collab::MapSubscription;
use collab::preclude::{Map, MapRefWrapper, Observable, ReadTxn, TransactionMut};
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
    subscription: Option<MapSubscription>,
}

impl ViewsMap {
    pub fn new(mut root: MapRefWrapper) -> ViewsMap {
        let subscription = root.observe(|txn, map_event| {});
        Self {
            container: root,
            subscription: Some(subscription),
        }
    }

    pub fn get_views(&self, view_ids: &[String]) -> Vec<View> {
        let txn = self.container.transact();
        self.get_views_with_txn(&txn, view_ids)
    }

    pub fn get_views_with_txn<T: ReadTxn>(&self, txn: &T, view_ids: &[String]) -> Vec<View> {
        view_ids
            .iter()
            .flat_map(|view_id| self.get_view_with_txn(txn, view_id, None))
            .collect::<Vec<_>>()
    }

    pub fn get_view(&self, view_id: &str, belong_to: Option<String>) -> Option<View> {
        let txn = self.container.transact();
        self.get_view_with_txn(&txn, view_id, belong_to)
    }

    pub fn get_view_with_txn<T: ReadTxn>(
        &self,
        txn: &T,
        view_id: &str,
        belong_to: Option<String>,
    ) -> Option<View> {
        let map_ref = self.container.get_map_with_txn(txn, view_id)?;
        let bid = map_ref.get_str_with_txn(txn, VIEW_BID)?;
        if let Some(belong_to) = belong_to {
            if belong_to != bid {
                return None;
            }
        }

        let id = map_ref.get_str_with_txn(txn, VIEW_ID)?;
        let name = map_ref.get_str_with_txn(txn, VIEW_NAME).unwrap_or_default();
        let desc = map_ref.get_str_with_txn(txn, VIEW_DESC).unwrap_or_default();
        let created_at = map_ref
            .get_i64_with_txn(txn, VIEW_CREATE_AT)
            .unwrap_or_default();
        let layout = map_ref
            .get_i64_with_txn(txn, VIEW_LAYOUT)
            .map(|value| value.try_into().ok())??;
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
        ViewBuilder::new(&view.id, txn, map_ref)
            .update(|update| {
                update
                    .set_name(view.name)
                    .set_bid(view.bid)
                    .set_desc(view.desc)
                    .set_layout(view.layout)
                    .set_created_at(view.created_at)
                    .set_belongings(view.belongings);
            })
            .done();
    }

    pub fn delete_view(&self, view_id: &str) {
        self.container
            .with_transact_mut(|txn| self.delete_view_with_txn(txn, view_id));
    }

    pub fn delete_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
        self.container.remove(txn, view_id);
    }

    pub fn update_view<F>(&self, view_id: &str, f: F) -> Result<()>
    where
        F: FnOnce(ViewUpdate),
    {
        self.container.with_transact_mut(|txn| {
            match self.container.get_map_with_txn(txn, view_id) {
                None => bail!("View is not existing"),
                Some(map_ref) => {
                    let update = ViewUpdate::new(txn, &map_ref);
                    f(update);
                    Ok(())
                }
            }
        })
    }
}

pub struct ViewBuilder<'a, 'b> {
    map_ref: MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b> ViewBuilder<'a, 'b> {
    pub fn new(view_id: &str, txn: &'a mut TransactionMut<'b>, map_ref: MapRefWrapper) -> Self {
        map_ref.insert_with_txn(txn, VIEW_ID, view_id);
        Self { map_ref, txn }
    }

    pub fn update<F>(self, f: F) -> Self
    where
        F: FnOnce(ViewUpdate),
    {
        let update = ViewUpdate::new(self.txn, &self.map_ref);
        f(update);
        self
    }
    pub fn done(self) {}
}

pub struct ViewUpdate<'a, 'b, 'c> {
    map_ref: &'c MapRefWrapper,
    txn: &'a mut TransactionMut<'b>,
}

impl<'a, 'b, 'c> ViewUpdate<'a, 'b, 'c> {
    pub fn new(txn: &'a mut TransactionMut<'b>, map_ref: &'c MapRefWrapper) -> Self {
        Self { map_ref, txn }
    }

    pub fn set_name<T: AsRef<str>>(self, name: T) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_NAME, name.as_ref());
        self
    }

    pub fn set_bid(self, bid: String) -> Self {
        self.map_ref.insert_with_txn(self.txn, VIEW_BID, bid);
        self
    }

    pub fn set_desc<T: AsRef<str>>(self, desc: T) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_DESC, desc.as_ref());
        self
    }

    pub fn set_layout(self, layout: ViewLayout) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_LAYOUT, layout as i64);
        self
    }

    pub fn set_created_at(self, created_at: i64) -> Self {
        self.map_ref
            .insert_with_txn(self.txn, VIEW_CREATE_AT, created_at);
        self
    }

    pub fn set_belongings(self, belongings: Belongings) -> Self {
        self.map_ref
            .insert_array_with_txn(self.txn, VIEW_BELONGINGS, belongings.into_inner());
        self
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct View {
    pub id: String,
    // bid short for belong to id
    pub bid: String,
    pub name: String,
    pub desc: String,
    pub belongings: Belongings,
    pub created_at: i64,
    pub layout: ViewLayout,
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ViewLayout {
    Document = 0,
    Grid = 1,
    Board = 2,
    Calendar = 3,
}

impl TryFrom<i64> for ViewLayout {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(ViewLayout::Document),
            1 => Ok(ViewLayout::Grid),
            2 => Ok(ViewLayout::Board),
            3 => Ok(ViewLayout::Calendar),
            _ => bail!("Unknown layout {}", value),
        }
    }
}

impl From<ViewLayout> for i64 {
    fn from(layout: ViewLayout) -> Self {
        layout as i64
    }
}
