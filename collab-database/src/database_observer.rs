use tokio::sync::broadcast;

use crate::rows::Row;
use crate::views::{FieldOrder, FilterMap, GroupMap, LayoutSetting, SortMap};

pub enum DatabaseViewChange {
  LayoutSettingChanged {
    view_id: String,
    setting: LayoutSetting,
  },
  // filter
  DidCreateFilter {
    view_id: String,
    filter: FilterMap,
  },
  DidDeleteFilter {
    view_id: String,
    filter: FilterMap,
  },
  DidUpdateFilter {
    view_id: String,
    filter: FilterMap,
  },
  // group
  DidCreateGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  DidDeleteGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  DidUpdateGroupSetting {
    view_id: String,
    group_setting: GroupMap,
  },
  // Sort
  DidCreateSort {
    view_id: String,
    sort: SortMap,
  },
  DidDeleteSort {
    view_id: String,
    sort: SortMap,
  },
  DidUpdateSort {
    view_id: String,
    sort: SortMap,
  },
  // field order
  DidCreateFieldOrder {
    view_id: String,
    field_order: FieldOrder,
  },
  DidDeleteFieldOrder {
    view_id: String,
    field_order: FieldOrder,
  },
}

pub enum RowChange {
  DidCreateRow { row: Row },
  DidDeleteRow { row: Row },
  DidUpdateRow { row: Row },
}

pub type RowChangeSender = broadcast::Sender<RowChange>;

#[derive(Clone)]
pub struct DatabaseNotify {
  pub view_change_tx: broadcast::Sender<DatabaseViewChange>,
  pub row_change_tx: RowChangeSender,
}
