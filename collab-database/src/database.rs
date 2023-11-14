use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use collab::core::any_map::AnyMapExtension;
use collab::core::collab::MutexCollab;
use collab::core::collab_state::{SnapshotState, SyncState};
use collab::preclude::{
  Collab, JsonValue, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
pub use collab_persistence::doc::YrsDocAction;
use collab_persistence::kv::rocks_kv::RocksCollabDB;
use nanoid::nanoid;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
pub use tokio_stream::wrappers::WatchStream;

use crate::blocks::{Block, BlockEvent};
use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::meta::MetaMap;
use crate::rows::{
  CreateRowParams, CreateRowParamsValidator, Row, RowCell, RowDetail, RowId, RowMeta,
  RowMetaUpdate, RowUpdate,
};
use crate::user::DatabaseCollabService;
use crate::views::{
  CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator, DatabaseLayout, DatabaseView,
  FieldOrder, FieldSettingsByFieldIdMap, FieldSettingsMap, FilterMap, GroupSettingMap,
  LayoutSetting, RowOrder, SortMap, ViewDescription, ViewMap,
};

pub struct Database {
  #[allow(dead_code)]
  inner: Arc<MutexCollab>,
  pub(crate) root: MapRefWrapper,
  pub views: Rc<ViewMap>,
  pub fields: Rc<FieldMap>,
  pub metas: Rc<MetaMap>,
  /// It used to keep track of the blocks. Each block contains a list of [Row]s
  /// A database rows will be stored in multiple blocks.
  pub block: Block,
}

const DATABASE_ID: &str = "id";
const DATABASE: &str = "database";
const FIELDS: &str = "fields";
const VIEWS: &str = "views";
const METAS: &str = "metas";

pub struct DatabaseContext {
  pub uid: i64,
  pub db: Weak<RocksCollabDB>,
  pub collab: Arc<MutexCollab>,
  pub collab_service: Arc<dyn DatabaseCollabService>,
}

impl Database {
  /// Create a new database with the given [CreateDatabaseParams]
  /// The method will set the inline view id to the given view_id
  /// from the [CreateDatabaseParams].
  pub fn create_with_inline_view(
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Self, DatabaseError> {
    // Get or create a empty database with the given database_id
    let this = Self::get_or_create(&params.database_id, context)?;
    let (rows, fields, params) = params.split();
    if !params.deps_fields.is_empty() {
      tracing::warn!("The deps_fields should be empty when creating a database with inline view.");
    }

    let row_orders = this.block.create_rows(rows);
    let field_orders = fields.iter().map(FieldOrder::from).collect();
    this.root.with_transact_mut(|txn| {
      // Set the inline view id. The inline view id should not be
      // empty if the current database exists.
      this.set_inline_view_with_txn(txn, &params.view_id);

      // Insert the given fields into the database
      for field in fields {
        this.fields.insert_field_with_txn(txn, field);
      }
      // Create a inline view
      this.create_view_with_txn(txn, params, field_orders, row_orders)?;
      Ok::<(), DatabaseError>(())
    })?;
    Ok(this)
  }

  pub fn load_all_rows(&self) {
    let row_ids = self
      .get_inline_row_orders()
      .into_iter()
      .map(|row_order| row_order.id)
      .take(100)
      .collect::<Vec<_>>();
    self.block.batch_load_rows(row_ids);
  }

  pub fn subscribe_block_event(&self) -> tokio::sync::broadcast::Receiver<BlockEvent> {
    self.block.subscribe_event()
  }

  /// Get or Create a database with the given database_id.
  pub fn get_or_create(database_id: &str, context: DatabaseContext) -> Result<Self, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    // Get the database from the collab
    let database = {
      let collab_guard = context.collab.lock();
      let txn = collab_guard.transact();
      collab_guard.get_map_with_txn(&txn, vec![DATABASE])
    };

    // If the database exists, return the database.
    // Otherwise, create a new database with the given database_id
    match database {
      None => Self::create(database_id, context),
      Some(database) => {
        let collab_guard = context.collab.lock();
        let (fields, views, metas) = collab_guard.with_origin_transact_mut(|txn| {
          // { DATABASE: { FIELDS: {:} } }
          let fields = collab_guard
            .get_map_with_txn(txn, vec![DATABASE, FIELDS])
            .unwrap();

          // { DATABASE: { FIELDS: {:}, VIEWS: {:} } }
          let views = collab_guard
            .get_map_with_txn(txn, vec![DATABASE, VIEWS])
            .unwrap();

          // { DATABASE: { FIELDS: {:},  VIEWS: {:}, METAS: {:} } }
          let metas = collab_guard
            .get_map_with_txn(txn, vec![DATABASE, METAS])
            .unwrap();

          (fields, views, metas)
        });
        let views = ViewMap::new(views);
        let fields = FieldMap::new(fields);
        let metas = MetaMap::new(metas);
        let block = Block::new(
          context.uid,
          context.db.clone(),
          context.collab_service.clone(),
        );
        drop(collab_guard);

        Ok(Self {
          inner: context.collab,
          root: database,
          block,
          views: Rc::new(views),
          fields: Rc::new(fields),
          metas: Rc::new(metas),
        })
      },
    }
  }

  /// Create a new database with the given database_id and context.
  fn create(database_id: &str, context: DatabaseContext) -> Result<Self, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }
    let collab_guard = context.collab.lock();
    let (database, fields, views, metas) = collab_guard.with_origin_transact_mut(|txn| {
      // { DATABASE: {:} }
      let database = collab_guard
        .get_map_with_txn(txn, vec![DATABASE])
        .unwrap_or_else(|| collab_guard.insert_map_with_txn(txn, DATABASE));

      database.insert_str_with_txn(txn, DATABASE_ID, database_id);

      // { DATABASE: { FIELDS: {:} } }
      let fields = collab_guard
        .get_map_with_txn(txn, vec![DATABASE, FIELDS])
        .unwrap_or_else(|| database.create_map_with_txn(txn, FIELDS));

      // { DATABASE: { FIELDS: {:}, VIEWS: {:} } }
      let views = collab_guard
        .get_map_with_txn(txn, vec![DATABASE, VIEWS])
        .unwrap_or_else(|| database.create_map_with_txn(txn, VIEWS));

      // { DATABASE: { FIELDS: {:},  VIEWS: {:}, METAS: {:} } }
      let metas = collab_guard
        .get_map_with_txn(txn, vec![DATABASE, METAS])
        .unwrap_or_else(|| database.create_map_with_txn(txn, METAS));

      (database, fields, views, metas)
    });
    drop(collab_guard);
    let views = ViewMap::new(views);
    let fields = FieldMap::new(fields);
    let metas = MetaMap::new(metas);

    let block = Block::new(
      context.uid,
      context.db.clone(),
      context.collab_service.clone(),
    );

    Ok(Self {
      inner: context.collab,
      root: database,
      block,
      views: Rc::new(views),
      fields: Rc::new(fields),
      metas: Rc::new(metas),
    })
  }

  pub fn close(&self) {
    let row_ids = self
      .get_inline_row_orders()
      .into_iter()
      .map(|row_order| row_order.id)
      .collect::<Vec<_>>();
    self.block.close_rows(&row_ids);
  }

  pub fn subscribe_sync_state(&self) -> WatchStream<SyncState> {
    self.inner.lock().subscribe_sync_state()
  }

  pub fn subscribe_snapshot_state(&self) -> WatchStream<SnapshotState> {
    self.inner.lock().subscribe_snapshot_state()
  }

  /// Return the database id
  pub fn get_database_id(&self) -> String {
    let txn = self.root.transact();
    // It's safe to unwrap. Because the database_id must exist
    self.root.get_str_with_txn(&txn, DATABASE_ID).unwrap()
  }

  /// Return the database id with a transaction
  pub fn get_database_id_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    self.root.get_str_with_txn(txn, DATABASE_ID).unwrap()
  }

  /// Create a new row from the given params.
  /// This row will be inserted to the end of rows of each view that
  /// reference the given database. Return the row order if the row is
  /// created successfully. Otherwise, return None.
  pub fn create_row(&self, params: CreateRowParams) -> Result<RowOrder, DatabaseError> {
    let params = CreateRowParamsValidator::validate(params)?;
    let row_order = self.block.create_row(params);
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.push_row_order(&row_order);
      });
    });
    Ok(row_order)
  }

  /// Create a new row from the given view.
  /// This row will be inserted into corresponding [Block]. The [RowOrder] of this row will
  /// be inserted to each view.
  pub fn create_row_in_view(
    &self,
    view_id: &str,
    params: CreateRowParams,
  ) -> Option<(usize, RowOrder)> {
    self
      .root
      .with_transact_mut(|txn| self.create_row_with_txn(txn, view_id, params))
  }

  /// Create a new row from the given view.
  /// This row will be inserted into corresponding [Block]. The [RowOrder] of this row will
  /// be inserted to each view.
  pub fn create_row_with_txn(
    &self,
    txn: &mut TransactionMut,
    view_id: &str,
    params: CreateRowParams,
  ) -> Option<(usize, RowOrder)> {
    let prev_row_id = params.prev_row_id.clone().map(|value| value.to_string());
    let row_order = self.block.create_row(params);
    self.views.update_all_views_with_txn(txn, |update| {
      update.insert_row_order(&row_order, prev_row_id.as_ref());
    });

    let index = self
      .index_of_row_with_txn(txn, view_id, row_order.id.clone())
      .unwrap_or_default();
    Some((index, row_order))
  }

  /// Remove the row
  /// The [RowOrder] of each view representing this row will be removed.
  pub fn remove_row(&self, row_id: &RowId) -> Option<Row> {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_row_order(row_id);
      });
      let row = self.block.get_row(row_id);
      self.block.delete_row(row_id);
      Some(row)
    })
  }

  /// Update the row
  pub fn update_row<F>(&self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    self.block.update_row(row_id, f);
  }

  /// Update the meta of the row
  pub fn update_row_meta<F>(&self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    self.block.update_row_meta(row_id, f);
  }

  /// Return the index of the row in the given view.
  /// Return None if the row is not found.
  pub fn index_of_row(&self, view_id: &str, row_id: &RowId) -> Option<usize> {
    let view = self.views.get_view(view_id)?;
    view.row_orders.iter().position(|order| &order.id == row_id)
  }

  pub fn index_of_row_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    row_id: RowId,
  ) -> Option<usize> {
    let view = self.views.get_view_with_txn(txn, view_id)?;
    view.row_orders.iter().position(|order| order.id == row_id)
  }

  /// Return the [Row] with the given row id.
  pub fn get_row(&self, row_id: &RowId) -> Row {
    self.block.get_row(row_id)
  }

  /// Return the [RowMeta] with the given row id.
  pub fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    self.block.get_row_meta(row_id)
  }

  /// Return the [RowMeta] with the given row id.
  pub fn get_row_detail(&self, row_id: &RowId) -> Option<RowDetail> {
    let row = self.block.get_row(row_id);
    let meta = self.block.get_row_meta(row_id)?;
    RowDetail::new(row, meta)
  }

  pub fn get_row_document_id(&self, row_id: &RowId) -> Option<String> {
    self.block.get_row_document_id(row_id)
  }

  /// Return a list of [Row] for the given view.
  /// The rows here are ordered by [RowOrder]s of the view.
  pub fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let row_orders = self.get_row_orders_for_view(view_id);
    self.get_rows_from_row_orders(&row_orders)
  }

  pub fn get_row_orders_for_view(&self, view_id: &str) -> Vec<RowOrder> {
    let txn = self.root.transact();
    self.views.get_row_orders_with_txn(&txn, view_id)
  }

  /// Return a list of [Row] for the given view.
  /// The rows here is ordered by the [RowOrder] of the view.
  pub fn get_rows_from_row_orders(&self, row_orders: &[RowOrder]) -> Vec<Row> {
    self.block.get_rows_from_row_orders(row_orders)
  }

  /// Return a list of [RowCell] for the given view and field.
  pub fn get_cells_for_field(&self, view_id: &str, field_id: &str) -> Vec<RowCell> {
    let txn = self.root.transact();
    self.get_cells_for_field_with_txn(&txn, view_id, field_id)
  }

  /// Return the [RowCell] with the given row id and field id.
  pub fn get_cell(&self, field_id: &str, row_id: &RowId) -> RowCell {
    let cell = self.block.get_cell(row_id, field_id);
    RowCell::new(row_id.clone(), cell)
  }

  /// Return list of [RowCell] for the given view and field.
  pub fn get_cells_for_field_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_id: &str,
  ) -> Vec<RowCell> {
    let row_orders = self.views.get_row_orders_with_txn(txn, view_id);
    let rows = self.block.get_rows_from_row_orders(&row_orders);
    rows
      .into_iter()
      .map(|row| RowCell::new(row.id, row.cells.get(field_id).cloned()))
      .collect::<Vec<RowCell>>()
  }

  /// Return the index of the field in the given view.
  pub fn index_of_field_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_id: &str,
  ) -> Option<usize> {
    let view = self.views.get_view_with_txn(txn, view_id)?;
    view
      .field_orders
      .iter()
      .position(|order| order.id == field_id)
  }

  /// Returns the [Field] with the given field ids.
  /// The fields are unordered.
  pub fn get_fields(&self, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.root.transact();
    self.get_fields_with_txn(&txn, field_ids)
  }

  pub fn get_fields_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    field_ids: Option<Vec<String>>,
  ) -> Vec<Field> {
    self.fields.get_fields_with_txn(txn, field_ids)
  }

  /// Get all fields in the database
  /// These fields are ordered by the [FieldOrder] of the view
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return the fields with the given ids
  pub fn get_fields_in_view(&self, view_id: &str, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.root.transact();
    self.get_fields_in_view_with_txn(&txn, view_id, field_ids)
  }

  /// Get all fields in the database
  /// These fields are ordered by the [FieldOrder] of the view
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return the fields with the given ids
  pub fn get_fields_in_view_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_ids: Option<Vec<String>>,
  ) -> Vec<Field> {
    let field_orders = self.views.get_field_orders_with_txn(txn, view_id);
    let mut all_field_map = self
      .fields
      .get_fields_with_txn(txn, field_ids)
      .into_iter()
      .map(|field| (field.id.clone(), field))
      .collect::<HashMap<String, Field>>();

    if field_orders.len() != all_field_map.len() {
      tracing::warn!(
        "🟡Field orders: {} and fields: {} are not the same length",
        field_orders.len(),
        all_field_map.len()
      );
    }

    field_orders
      .into_iter()
      .flat_map(|order| all_field_map.remove(&order.id))
      .collect()
  }

  /// Creates a new field, appends the field_id to the end of field order
  /// and adds a field setting
  pub fn create_field(
    &self,
    field: Field,
    field_settings_by_layout: HashMap<DatabaseLayout, FieldSettingsMap>,
  ) {
    self.root.with_transact_mut(|txn| {
      self.create_field_with_txn(txn, field, &field_settings_by_layout);
    })
  }

  pub fn create_field_with_txn(
    &self,
    txn: &mut TransactionMut,
    field: Field,
    field_settings_by_layout: &HashMap<DatabaseLayout, FieldSettingsMap>,
  ) {
    self.views.update_all_views_with_txn(txn, |update| {
      let field_id = field.id.clone();
      update
        .push_field_order(&field)
        .update_field_settings_for_fields(
          vec![field_id],
          |field_id, field_setting_update, layout_ty| {
            field_setting_update.update(
              field_id,
              field_settings_by_layout.get(&layout_ty).unwrap().clone(),
            );
          },
        );
    });
    self.fields.insert_field_with_txn(txn, field);
  }

  pub fn create_field_with_mut(
    &self,
    view_id: &str,
    name: String,
    field_type: i64,
    f: impl FnOnce(&mut Field),
    field_settings_by_layout: HashMap<DatabaseLayout, FieldSettingsMap>,
  ) -> (usize, Field) {
    let mut field = Field::new(gen_field_id(), name, field_type, false);
    f(&mut field);
    let index = self.root.with_transact_mut(|txn| {
      self.create_field_with_txn(txn, field.clone(), &field_settings_by_layout);
      self
        .index_of_field_with_txn(txn, view_id, &field.id)
        .unwrap_or_default()
    });

    (index, field)
  }

  /// Creates a new field, add a field setting, but inserts the field after a
  /// certain field_id
  fn insert_field_with_txn(
    &self,
    txn: &mut TransactionMut,
    field: Field,
    prev_field_id: Option<String>,
  ) {
    self.views.update_all_views_with_txn(txn, |update| {
      update.insert_field_order(&field, prev_field_id.as_ref());
    });
    self.fields.insert_field_with_txn(txn, field);
  }

  pub fn delete_field(&self, field_id: &str) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update
          .remove_field_order(field_id)
          .remove_field_setting(field_id);
      });
      self.fields.delete_field_with_txn(txn, field_id);
    })
  }

  pub fn get_all_group_setting<T: TryFrom<GroupSettingMap>>(&self, view_id: &str) -> Vec<T> {
    self
      .views
      .get_view_group_setting(view_id)
      .into_iter()
      .flat_map(|setting| T::try_from(setting).ok())
      .collect()
  }

  /// Add a group setting to the view. If the setting already exists, it will be replaced.
  pub fn insert_group_setting(&self, view_id: &str, group_setting: impl Into<GroupSettingMap>) {
    self.views.update_database_view(view_id, |update| {
      update.update_groups(|group_update| {
        let group_setting = group_setting.into();
        if let Some(setting_id) = group_setting.get_str_value("id") {
          if group_update.contains(&setting_id) {
            group_update.update(&setting_id, |_| group_setting);
          } else {
            group_update.push(group_setting);
          }
        } else {
          group_update.push(group_setting);
        }
      });
    });
  }

  pub fn delete_group_setting(&self, view_id: &str, group_setting_id: &str) {
    self.views.update_database_view(view_id, |update| {
      update.update_groups(|group_update| {
        group_update.remove(group_setting_id);
      });
    });
  }

  pub fn update_group_setting(
    &self,
    view_id: &str,
    setting_id: &str,
    f: impl FnOnce(&mut GroupSettingMap),
  ) {
    self.views.update_database_view(view_id, |view_update| {
      view_update.update_groups(|group_update| {
        group_update.update(setting_id, |mut map| {
          f(&mut map);
          map
        });
      });
    });
  }

  pub fn remove_group_setting(&self, view_id: &str, setting_id: &str) {
    self.views.update_database_view(view_id, |update| {
      update.update_groups(|group_update| {
        group_update.remove(setting_id);
      });
    });
  }

  pub fn insert_sort(&self, view_id: &str, sort: impl Into<SortMap>) {
    self.views.update_database_view(view_id, |update| {
      update.update_sorts(|sort_update| {
        let sort = sort.into();
        if let Some(sort_id) = sort.get_str_value("id") {
          if sort_update.contains(&sort_id) {
            sort_update.update(&sort_id, |_| sort);
          } else {
            sort_update.push(sort);
          }
        } else {
          sort_update.push(sort);
        }
      });
    });
  }

  pub fn get_all_sorts<T: TryFrom<SortMap>>(&self, view_id: &str) -> Vec<T> {
    self
      .views
      .get_view_sorts(view_id)
      .into_iter()
      .flat_map(|sort| T::try_from(sort).ok())
      .collect()
  }

  pub fn get_sort<T: TryFrom<SortMap>>(&self, view_id: &str, sort_id: &str) -> Option<T> {
    let sort_id = sort_id.to_string();
    let mut sorts = self
      .views
      .get_view_sorts(view_id)
      .into_iter()
      .filter(|filter_map| filter_map.get_str_value("id").as_ref() == Some(&sort_id))
      .flat_map(|value| T::try_from(value).ok())
      .collect::<Vec<T>>();
    if sorts.is_empty() {
      None
    } else {
      Some(sorts.remove(0))
    }
  }

  pub fn remove_sort(&self, view_id: &str, sort_id: &str) {
    self.views.update_database_view(view_id, |update| {
      update.update_sorts(|sort_update| {
        sort_update.remove(sort_id);
      });
    });
  }

  pub fn remove_all_sorts(&self, view_id: &str) {
    self.views.update_database_view(view_id, |update| {
      update.update_sorts(|sort_update| {
        sort_update.clear();
      });
    });
  }

  pub fn get_all_filters<T: TryFrom<FilterMap>>(&self, view_id: &str) -> Vec<T> {
    self
      .views
      .get_view_filters(view_id)
      .into_iter()
      .flat_map(|setting| T::try_from(setting).ok())
      .collect()
  }

  pub fn get_filter<T: TryFrom<FilterMap>>(&self, view_id: &str, filter_id: &str) -> Option<T> {
    let filter_id = filter_id.to_string();
    let mut filters = self
      .views
      .get_view_filters(view_id)
      .into_iter()
      .filter(|filter_map| filter_map.get_str_value("id").as_ref() == Some(&filter_id))
      .flat_map(|value| T::try_from(value).ok())
      .collect::<Vec<T>>();
    if filters.is_empty() {
      None
    } else {
      Some(filters.remove(0))
    }
  }

  pub fn get_filter_by_field_id<T: TryFrom<FilterMap>>(
    &self,
    view_id: &str,
    field_id: &str,
  ) -> Option<T> {
    let field_id = field_id.to_string();
    let mut filters = self
      .views
      .get_view_filters(view_id)
      .into_iter()
      .filter(|filter_map| filter_map.get_str_value("field_id").as_ref() == Some(&field_id))
      .flat_map(|value| T::try_from(value).ok())
      .collect::<Vec<T>>();
    if filters.is_empty() {
      None
    } else {
      Some(filters.remove(0))
    }
  }

  pub fn update_filter(&self, view_id: &str, filter_id: &str, f: impl FnOnce(&mut FilterMap)) {
    self.views.update_database_view(view_id, |view_update| {
      view_update.update_filters(|filter_update| {
        filter_update.update(filter_id, |mut map| {
          f(&mut map);
          map
        });
      });
    });
  }

  pub fn remove_filter(&self, view_id: &str, filter_id: &str) {
    self.views.update_database_view(view_id, |update| {
      update.update_filters(|filter_update| {
        filter_update.remove(filter_id);
      });
    });
  }

  /// Add a filter to the view. If the setting already exists, it will be replaced.
  pub fn insert_filter(&self, view_id: &str, filter: impl Into<FilterMap>) {
    self.views.update_database_view(view_id, |update| {
      update.update_filters(|filter_update| {
        let filter = filter.into();
        if let Some(filter_id) = filter.get_str_value("id") {
          if filter_update.contains(&filter_id) {
            filter_update.update(&filter_id, |_| filter);
          } else {
            filter_update.push(filter);
          }
        } else {
          filter_update.push(filter);
        }
      });
    });
  }

  pub fn get_layout_setting<T: From<LayoutSetting>>(
    &self,
    view_id: &str,
    layout_ty: &DatabaseLayout,
  ) -> Option<T> {
    self.views.get_layout_setting(view_id, layout_ty)
  }

  pub fn insert_layout_setting<T: Into<LayoutSetting>>(
    &self,
    view_id: &str,
    layout_ty: &DatabaseLayout,
    layout_setting: T,
  ) {
    self.views.update_database_view(view_id, |update| {
      update.update_layout_settings(layout_ty, layout_setting.into());
    });
  }

  /// Returns the field settings for the given field ids.
  /// If None, return field settings for all fields
  pub fn get_field_settings<T: From<FieldSettingsMap>>(
    &self,
    view_id: &str,
    field_ids: Option<&[String]>,
  ) -> HashMap<String, T> {
    let mut field_settings_map = self
      .views
      .get_view_field_settings(view_id)
      .into_inner()
      .into_iter()
      .map(|(field_id, field_setting)| (field_id, T::from(field_setting)))
      .collect::<HashMap<String, T>>();

    if let Some(field_ids) = field_ids {
      field_settings_map.retain(|field_id, _| field_ids.contains(field_id));
    }

    field_settings_map
  }

  pub fn set_field_settings(&self, view_id: &str, field_settings_map: FieldSettingsByFieldIdMap) {
    self.views.update_database_view(view_id, |update| {
      update.set_field_settings(field_settings_map);
    })
  }

  pub fn update_field_settings(
    &self,
    view_id: &str,
    field_ids: Option<Vec<String>>,
    field_settings: impl Into<FieldSettingsMap>,
  ) {
    let field_ids = field_ids.unwrap_or(
      self
        .get_fields(None)
        .into_iter()
        .map(|field| field.id)
        .collect(),
    );
    self.views.update_database_view(view_id, |update| {
      let field_settings = field_settings.into();
      update.update_field_settings_for_fields(
        field_ids,
        |field_id, field_setting_update, _layout_ty| {
          field_setting_update.update(field_id, field_settings.clone());
        },
      );
    })
  }

  pub fn remove_field_settings_for_fields(&self, view_id: &str, field_ids: Vec<String>) {
    self.views.update_database_view(view_id, |update| {
      update.update_field_settings_for_fields(
        field_ids,
        |field_id, field_setting_update, _layout_ty| {
          field_setting_update.remove(field_id);
        },
      );
    })
  }

  /// Update the layout type of the view.
  pub fn update_layout_type(&self, view_id: &str, layout_type: &DatabaseLayout) {
    self.views.update_database_view(view_id, |update| {
      update.set_layout_type(*layout_type);
    });
  }

  /// Returns all the views that the current database has.
  pub fn get_all_views_description(&self) -> Vec<ViewDescription> {
    let txn = self.root.transact();
    self.views.get_all_views_description_with_txn(&txn)
  }

  /// Create a linked view to existing database
  pub fn create_linked_view(&self, params: CreateViewParams) -> Result<(), DatabaseError> {
    let mut params = CreateViewParamsValidator::validate(params)?;
    self.root.with_transact_mut(|txn| {
      let inline_view_id = self.get_inline_view_id_with_txn(txn);
      let row_orders = self.views.get_row_orders_with_txn(txn, &inline_view_id);
      let field_orders = self.views.get_field_orders_with_txn(txn, &inline_view_id);
      let (deps_fields, deps_field_settings) = params.take_deps_fields();

      self.create_view_with_txn(txn, params, field_orders, row_orders)?;

      // After creating the view, we need to create the fields that are used in the view.
      if !deps_fields.is_empty() {
        tracing::trace!("create linked view with deps fields: {:?}", deps_fields);
        deps_fields
          .into_iter()
          .zip(deps_field_settings.into_iter())
          .for_each(|(field, field_settings)| {
            self.create_field_with_txn(txn, field, &field_settings);
          })
      }
      Ok::<(), DatabaseError>(())
    })?;
    Ok(())
  }

  /// Create a [DatabaseView] for the current database.
  pub fn create_view_with_txn(
    &self,
    txn: &mut TransactionMut,
    params: CreateViewParams,
    field_orders: Vec<FieldOrder>,
    row_orders: Vec<RowOrder>,
  ) -> Result<(), DatabaseError> {
    let params = CreateViewParamsValidator::validate(params)?;
    let timestamp = timestamp();
    let database_id = self.get_database_id_with_txn(txn);
    let view = DatabaseView {
      id: params.view_id,
      database_id,
      name: params.name,
      layout: params.layout,
      layout_settings: params.layout_settings,
      filters: params.filters,
      group_settings: params.groups,
      sorts: params.sorts,
      field_settings: params.field_settings,
      row_orders,
      field_orders,
      created_at: timestamp,
      modified_at: timestamp,
    };
    // tracing::trace!("create linked view with params {:?}", params);
    self.views.insert_view_with_txn(txn, view);
    Ok(())
  }

  /// Create a linked view that duplicate the target view's setting including filter, sort,
  /// group, field setting, etc.
  pub fn duplicate_linked_view(&self, view_id: &str) -> Option<DatabaseView> {
    let view = self.views.get_view(view_id)?;
    let mut duplicated_view = view.clone();
    duplicated_view.id = gen_database_view_id();
    duplicated_view.created_at = timestamp();
    duplicated_view.modified_at = timestamp();
    duplicated_view.name = format!("{}-copy", view.name);
    self.views.insert_view(duplicated_view.clone());

    Some(duplicated_view)
  }

  /// Duplicate the row, and insert it after the original row.
  pub fn duplicate_row(&self, row_id: &RowId) -> Option<CreateRowParams> {
    let row = self.block.get_row(row_id);
    Some(CreateRowParams {
      id: gen_row_id(),
      cells: row.cells,
      height: row.height,
      visibility: row.visibility,
      prev_row_id: Some(row.id),
      timestamp: timestamp(),
    })
  }

  pub fn duplicate_field(
    &self,
    view_id: &str,
    field_id: &str,
    f: impl FnOnce(&Field) -> String,
  ) -> Option<(usize, Field)> {
    self.root.with_transact_mut(|txn| {
      if let Some(mut field) = self.fields.get_field_with_txn(txn, field_id) {
        field.id = gen_field_id();
        field.name = f(&field);
        self.insert_field_with_txn(txn, field.clone(), Some(field_id.to_string()));
        let index = self
          .index_of_field_with_txn(txn, view_id, &field.id)
          .unwrap_or_default();
        Some((index, field))
      } else {
        None
      }
    })
  }

  pub fn duplicate_database(&self) -> DatabaseData {
    let inline_view_id = self.get_inline_view_id();
    let txn = self.root.transact();
    let timestamp = timestamp();
    let mut view = self.views.get_view_with_txn(&txn, &inline_view_id).unwrap();
    let fields = self.get_fields_in_view_with_txn(&txn, &inline_view_id, None);
    let row_orders = self.views.get_row_orders_with_txn(&txn, &view.id);
    let rows = self
      .block
      .get_rows_from_row_orders(&row_orders)
      .into_iter()
      .map(|row| CreateRowParams {
        id: gen_row_id(),
        cells: row.cells,
        height: row.height,
        visibility: row.visibility,
        prev_row_id: None,
        timestamp,
      })
      .collect::<Vec<CreateRowParams>>();

    view.id = gen_database_view_id();
    view.database_id = gen_database_id();
    DatabaseData { view, fields, rows }
  }

  pub fn get_view(&self, view_id: &str) -> Option<DatabaseView> {
    let txn = self.root.transact();
    self.views.get_view_with_txn(&txn, view_id)
  }

  pub fn to_json_value(&self) -> JsonValue {
    let database_serde = DatabaseSerde::from_database(self);
    serde_json::to_value(&database_serde).unwrap()
  }

  pub fn is_inline_view(&self, view_id: &str) -> bool {
    let inline_view_id = self.get_inline_view_id();
    inline_view_id == view_id
  }

  pub fn get_database_rows(&self) -> Vec<Row> {
    let row_orders = {
      let txn = self.root.transact();
      let inline_view_id = self.get_inline_view_id_with_txn(&txn);
      self.views.get_row_orders_with_txn(&txn, &inline_view_id)
    };

    self.get_rows_from_row_orders(&row_orders)
  }

  pub fn get_inline_row_orders(&self) -> Vec<RowOrder> {
    let collab = self.inner.lock();
    let txn = collab.transact();
    let inline_view_id = self.get_inline_view_id_with_txn(&txn);
    self.views.get_row_orders_with_txn(&txn, &inline_view_id)
  }

  pub fn set_inline_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
    tracing::trace!("Set inline view id: {}", view_id);
    self.metas.set_inline_view_with_txn(txn, view_id);
  }

  /// The inline view is the view that create with the database when initializing
  pub fn get_inline_view_id(&self) -> String {
    let txn = self.root.transact();
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    self.metas.get_inline_view_with_txn(&txn).unwrap()
  }

  fn get_inline_view_id_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    self.metas.get_inline_view_with_txn(txn).unwrap()
  }

  /// Delete a view from the database and returns the deleted view ids.
  /// If the view is the inline view, it will clear all the views. Otherwise,
  /// just delete the view with given view id.
  ///
  pub fn delete_view(&self, view_id: &str) -> Vec<String> {
    if self.is_inline_view(view_id) {
      self.root.with_transact_mut(|txn| {
        let views = self.views.get_all_views_description_with_txn(txn);
        self.views.clear_with_txn(txn);
        views.into_iter().map(|view| view.id).collect()
      })
    } else {
      self.root.with_transact_mut(|txn| {
        self.views.delete_view_with_txn(txn, view_id);
      });
      vec![view_id.to_string()]
    }
  }

  /// Only expose this function in test env
  #[cfg(debug_assertions)]
  pub fn get_mutex_collab(&self) -> &Arc<MutexCollab> {
    &self.inner
  }
}

pub fn gen_database_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

pub fn gen_database_view_id() -> String {
  format!("v:{}", nanoid!(6))
}

pub fn gen_field_id() -> String {
  nanoid!(6)
}

pub fn gen_row_id() -> RowId {
  RowId::from(uuid::Uuid::new_v4().to_string())
}

pub fn gen_database_filter_id() -> String {
  nanoid!(6)
}

pub fn gen_database_group_id() -> String {
  format!("g:{}", nanoid!(6))
}

pub fn gen_database_sort_id() -> String {
  format!("s:{}", nanoid!(6))
}

pub fn gen_option_id() -> String {
  nanoid!(4)
}

pub fn timestamp() -> i64 {
  chrono::Utc::now().timestamp()
}

/// DatabaseData contains all the data of a database.
/// It's used to export and import a database. For example, duplicating a database
#[derive(Clone, Serialize, Deserialize)]
pub struct DatabaseData {
  pub view: DatabaseView,
  pub fields: Vec<Field>,
  pub rows: Vec<CreateRowParams>,
}

impl DatabaseData {
  pub fn to_json(&self) -> Result<String, DatabaseError> {
    let s = serde_json::to_string(self)?;
    Ok(s)
  }

  pub fn from_json(json: &str) -> Result<Self, DatabaseError> {
    let database = serde_json::from_str(json)?;
    Ok(database)
  }

  pub fn to_json_bytes(&self) -> Result<Vec<u8>, DatabaseError> {
    Ok(self.to_json()?.as_bytes().to_vec())
  }

  pub fn from_json_bytes(json: Vec<u8>) -> Result<Self, DatabaseError> {
    let database = serde_json::from_slice(&json)?;
    Ok(database)
  }
}

#[derive(Clone)]
pub struct MutexDatabase(Arc<Mutex<Database>>);

impl MutexDatabase {
  pub fn new(inner: Database) -> Self {
    Self(Arc::new(Mutex::new(inner)))
  }
}

impl Deref for MutexDatabase {
  type Target = Arc<Mutex<Database>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

unsafe impl Sync for MutexDatabase {}

unsafe impl Send for MutexDatabase {}

pub fn get_database_row_ids(collab: &Collab) -> Option<Vec<String>> {
  let txn = collab.transact();
  let views = collab.get_map_with_txn(&txn, vec![DATABASE, VIEWS])?;
  let metas = collab.get_map_with_txn(&txn, vec![DATABASE, METAS])?;

  let views = ViewMap::new(views);
  let meta = MetaMap::new(metas);

  let inline_view_id = meta.get_inline_view_with_txn(&txn)?;
  Some(
    views
      .get_row_orders_with_txn(&txn, &inline_view_id)
      .into_iter()
      .map(|order| order.id.to_string())
      .collect(),
  )
}

pub fn reset_inline_view_id<F>(collab: &Collab, f: F)
where
  F: Fn(String) -> String,
{
  collab.with_origin_transact_mut(|txn| {
    if let Some(container) = collab.get_map_with_txn(txn, vec![DATABASE, METAS]) {
      let map = MetaMap::new(container);
      let inline_view_id = map.get_inline_view_with_txn(txn).unwrap();
      let new_inline_view_id = f(inline_view_id);
      map.set_inline_view_with_txn(txn, &new_inline_view_id);
    }
  })
}

pub fn mut_database_views_with_collab<F>(collab: &Collab, f: F)
where
  F: Fn(&mut DatabaseView),
{
  collab.with_origin_transact_mut(|txn| {
    if let Some(container) = collab.get_map_with_txn(txn, vec![DATABASE, VIEWS]) {
      let views = ViewMap::new(container);
      let mut reset_views = views.get_all_views_with_txn(txn);

      reset_views.iter_mut().for_each(f);
      for view in reset_views {
        views.insert_view_with_txn(txn, view);
      }
    }
  });
}

pub fn is_database_collab(collab: &Collab) -> bool {
  let txn = collab.transact();
  collab.get_map_with_txn(&txn, vec![DATABASE]).is_some()
}
