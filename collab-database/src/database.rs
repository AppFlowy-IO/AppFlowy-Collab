use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

use crate::blocks::{Block, BlockEvent, InitRowChan};
use crate::database_state::DatabaseNotify;
use crate::error::DatabaseError;
use crate::fields::{
  Field, FieldChangeReceiver, FieldMap, FieldUpdate, TypeOptionCellReader, TypeOptionCellWriter,
  type_option_cell_reader, type_option_cell_writer,
};
use crate::meta::MetaMap;
use crate::rows::{
  CreateRowParams, CreateRowParamsValidator, DatabaseRow, Row, RowCell, RowChangeReceiver,
  RowDetail, RowId, RowMeta, RowMetaKey, RowMetaUpdate, RowUpdate, meta_id_from_row_id,
};
use crate::util::encoded_collab;
use crate::views::define::DATABASE_VIEW_ROW_ORDERS;
use crate::views::{
  CalculationMap, DatabaseLayout, DatabaseViewUpdate, DatabaseViews, FieldOrder,
  FieldSettingsByFieldIdMap, FieldSettingsMap, FilterMap, GroupSettingMap, LayoutSetting,
  OrderArray, OrderObjectPosition, RowOrder, RowOrderArray, SortMap, ViewChangeReceiver,
};
use crate::workspace_database::DatabaseMeta;

use crate::entity::{
  CreateDatabaseParams, CreateViewParams, CreateViewParamsValidator, DatabaseView,
  DatabaseViewMeta, EncodedCollabInfo, EncodedDatabase, FieldType,
};
use crate::template::entity::DatabaseTemplate;

use collab::core::origin::CollabOrigin;
use collab::lock::RwLock;
use collab::preclude::{
  Any, Array, Collab, FillRef, JsonValue, Map, MapExt, MapPrelim, MapRef, ReadTxn, ToJson,
  TransactionMut, YrsValue,
};
use collab::util::{AnyExt, ArrayExt};
use collab_entity::CollabType;
use collab_entity::define::{DATABASE, DATABASE_ID, DATABASE_METAS};

use futures::stream::StreamExt;
use futures::{Stream, stream};
use nanoid::nanoid;

use crate::database_trait::{DatabaseCollabService, DatabaseDataVariant, DatabaseRowCollabService};
use collab::core::collab::CollabOptions;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, trace};
use uuid::Uuid;
use yrs::block::ClientID;

pub struct Database {
  pub collab: Collab,
  pub body: DatabaseBody,
  pub collab_service: Arc<dyn DatabaseCollabService>,
}
impl Drop for Database {
  fn drop(&mut self) {
    #[cfg(feature = "verbose_log")]
    trace!("Database dropped: {}", self.collab.object_id());
  }
}

const FIELDS: &str = "fields";
const VIEWS: &str = "views";

#[derive(Clone)]
pub struct DatabaseContext {
  pub database_collab_service: Arc<dyn DatabaseCollabService>,
  pub database_row_collab_service: Arc<dyn DatabaseRowCollabService>,
  pub notifier: DatabaseNotify,
}

impl DatabaseContext {
  pub fn new(
    database_collab_service: Arc<dyn DatabaseCollabService>,
    database_row_collab_service: Arc<dyn DatabaseRowCollabService>,
  ) -> Self {
    Self {
      database_collab_service,
      database_row_collab_service,
      notifier: DatabaseNotify::default(),
    }
  }
}

pub async fn default_database_collab(
  database_id: &str,
  client_id: ClientID,
  params: Option<CreateDatabaseParams>,
  context: DatabaseContext,
) -> Result<(DatabaseBody, Collab), DatabaseError> {
  let collab = Collab::new_with_options(
    CollabOrigin::Empty,
    CollabOptions::new(database_id.to_string(), client_id),
  )
  .map_err(|e| DatabaseError::Internal(e.into()))?;
  let collab = match params {
    None => {
      DatabaseBody::create(
        collab,
        database_id.to_string(),
        context,
        vec![],
        vec![],
        vec![],
      )
      .await?
    },
    Some(params) => {
      DatabaseBody::create(
        collab,
        database_id.to_string(),
        context,
        params.rows,
        params.fields,
        params.views,
      )
      .await?
    },
  };

  Ok(collab)
}

impl Database {
  /// Get or Create a database with the given database_id.
  #[instrument(level = "trace", skip_all, err)]
  pub async fn arc_open(
    database_id: &str,
    context: DatabaseContext,
  ) -> Result<Arc<RwLock<Self>>, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    let collab = context
      .database_collab_service
      .clone()
      .build_arc_database(database_id, false, None, context)
      .await?;

    Ok(collab)
  }

  pub async fn open(database_id: &str, context: DatabaseContext) -> Result<Self, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    let database = context
      .database_collab_service
      .clone()
      .build_database(database_id, false, None, context)
      .await?;

    Ok(database)
  }

  pub async fn create(
    context: DatabaseContext,
    params: CreateDatabaseParams,
  ) -> Result<Self, DatabaseError> {
    if params.database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }
    trace!(
      "[Database] create {}, client_id: {}",
      params.database_id,
      context.database_collab_service.database_client_id().await
    );
    let database_id = params.database_id.clone();
    let database = context
      .database_collab_service
      .clone()
      .build_database(
        &database_id,
        true,
        Some(DatabaseDataVariant::Params(params)),
        context,
      )
      .await?;
    Ok(database)
  }

  #[instrument(level = "trace", skip_all, err)]
  pub async fn arc_create(
    context: DatabaseContext,
    params: CreateDatabaseParams,
  ) -> Result<Arc<RwLock<Self>>, DatabaseError> {
    if params.database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID("database_id is empty"));
    }

    trace!(
      "[Database] create {}, client_id: {}",
      params.database_id,
      context.database_collab_service.database_client_id().await
    );
    let database_id = params.database_id.clone();
    let database = context
      .database_collab_service
      .clone()
      .build_arc_database(
        &database_id,
        true,
        Some(DatabaseDataVariant::Params(params)),
        context,
      )
      .await?;
    Ok(database)
  }

  pub async fn create_with_template<T>(
    template: T,
    database_collab_service: Arc<dyn DatabaseCollabService>,
    database_row_collab_service: Arc<dyn DatabaseRowCollabService>,
  ) -> Result<Database, DatabaseError>
  where
    T: TryInto<DatabaseTemplate> + Send + Sync + 'static,
    <T as TryInto<DatabaseTemplate>>::Error: ToString,
  {
    let params = tokio::task::spawn_blocking(move || {
      template
        .try_into()
        .map_err(|err| DatabaseError::ImportData(err.to_string()))
    })
    .await
    .map_err(|e| DatabaseError::Internal(e.into()))??
    .into_params();

    let context = DatabaseContext {
      database_collab_service,
      database_row_collab_service,
      notifier: Default::default(),
    };
    Self::create_with_view(params, context).await
  }

  /// Create a new database with the given [CreateDatabaseParams]
  /// The method will set the inline view id to the given view_id
  /// from the [CreateDatabaseParams].
  pub async fn create_arc_with_view(
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Arc<RwLock<Database>>, DatabaseError> {
    let database = Self::arc_create(context, params).await?;
    Ok(database)
  }

  pub async fn create_with_view(
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Database, DatabaseError> {
    let database = Self::create(context, params).await?;
    Ok(database)
  }

  pub async fn encode_database_collabs(&self) -> Result<EncodedDatabase, DatabaseError> {
    let database_id = Uuid::parse_str(self.collab.object_id())?;
    let encoded_database_collab = EncodedCollabInfo {
      object_id: database_id,
      collab_type: CollabType::Database,
      encoded_collab: encoded_collab(&self.collab, &CollabType::Database)?,
    };

    // Fetch row orders
    let row_orders = self.get_all_row_orders().await;
    let mut encoded_row_collabs = Vec::new();
    // Process row orders in chunks
    for chunk in row_orders.chunks(20) {
      // Create async tasks for each row in the chunk
      let tasks: Vec<_> = chunk
        .iter()
        .map(|chunk_row| async move {
          let database_row = self
            .body
            .block
            .get_or_init_database_row(&chunk_row.id)
            .await
            .ok()?;
          let read_guard = database_row.read().await;
          let row_collab = &read_guard.collab;
          let object_id = Uuid::parse_str(row_collab.object_id()).ok()?;
          let encoded_collab = encoded_collab(row_collab, &CollabType::DatabaseRow).ok()?;
          Some(EncodedCollabInfo {
            object_id,
            collab_type: CollabType::DatabaseRow,
            encoded_collab,
          })
        })
        .collect();

      let chunk_results = join_all(tasks).await;
      for collab_info in chunk_results.into_iter().flatten() {
        encoded_row_collabs.push(collab_info);
      }

      // Yield to the runtime after processing each chunk
      tokio::task::yield_now().await;
    }

    Ok(EncodedDatabase {
      encoded_database_collab,
      encoded_row_collabs,
    })
  }

  pub fn validate(&self) -> Result<(), DatabaseError> {
    CollabType::Database.validate_require_data(&self.collab)?;
    Ok(())
  }

  pub fn subscribe_row_change(&self) -> Option<RowChangeReceiver> {
    self
      .body
      .notifier
      .as_ref()
      .map(|notifier| notifier.row_change_tx.subscribe())
  }

  pub fn subscribe_field_change(&self) -> Option<FieldChangeReceiver> {
    self
      .body
      .notifier
      .as_ref()
      .map(|notifier| notifier.field_change_tx.subscribe())
  }

  pub fn subscribe_view_change(&self) -> Option<ViewChangeReceiver> {
    self
      .body
      .notifier
      .as_ref()
      .map(|notifier| notifier.view_change_tx.subscribe())
  }

  pub fn subscribe_block_event(&self) -> tokio::sync::broadcast::Receiver<BlockEvent> {
    self.body.block.subscribe_event()
  }

  /// Return all field orders without order
  pub fn get_all_field_orders(&self) -> Vec<FieldOrder> {
    let txn = self.collab.transact();
    match self.body.try_get_inline_view_id(&txn) {
      None => {
        error!("Cannot find inline view id");
        self.body.fields.get_all_field_orders(&txn)
      },
      Some(inline_view_id) => self.body.views.get_field_orders(&txn, &inline_view_id),
    }
  }

  pub fn get_all_views(&self) -> Vec<DatabaseView> {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_all_views(&txn)
      .into_iter()
      .filter(|view| !view.is_inline)
      .collect()
  }

  pub fn get_database_view_layout(&self, view_id: &str) -> DatabaseLayout {
    let txn = self.collab.transact();
    self.body.views.get_database_view_layout(&txn, view_id)
  }

  /// Return the database id with a transaction
  pub fn get_database_id(&self) -> String {
    let txn = self.collab.transact();
    self.body.get_database_id(&txn)
  }

  /// Create a new row from the given params.
  /// This row will be inserted to the end of rows of each view that
  /// reference the given database. Return the row order if the row is
  /// created successfully. Otherwise, return None.
  pub async fn create_row(&mut self, params: CreateRowParams) -> Result<RowOrder, DatabaseError> {
    let client_id = self.collab_service.database_client_id().await;
    let params = CreateRowParamsValidator::validate(params)?;
    let row_order = self.body.block.create_new_row(params, client_id).await?;
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_all_views(&mut txn, |_view_id, update| {
        update.insert_row_order(&row_order, &OrderObjectPosition::default());
      });
    Ok(row_order)
  }

  pub fn update_database_view<F>(&mut self, view_id: &str, f: F)
  where
    F: FnOnce(DatabaseViewUpdate),
  {
    let mut txn = self.collab.transact_mut();
    self.body.views.update_database_view(&mut txn, view_id, f);
  }

  pub fn contains_row(&self, view_id: &str, row_id: &RowId) -> bool {
    let txn = self.collab.transact();
    if let Some(YrsValue::YMap(view)) = self.body.views.get(&txn, view_id) {
      if let Some(YrsValue::YArray(row_orders)) = view.get(&txn, DATABASE_VIEW_ROW_ORDERS) {
        return RowOrderArray::new(row_orders)
          .get_position_with_txn(&txn, row_id)
          .is_some();
      }
    }
    false
  }

  /// Create a new row from the given view.
  /// This row will be inserted into corresponding [Block]. The [RowOrder] of this row will
  /// be inserted to each view.
  pub async fn create_row_in_view(
    &mut self,
    view_id: &str,
    params: CreateRowParams,
  ) -> Result<(usize, RowOrder), DatabaseError> {
    let client_id = self.collab_service.database_client_id().await;
    let row_position = params.row_position.clone();
    let row_order = self.body.create_row(params, client_id).await?;

    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_all_views(&mut txn, |_view_id, update| {
        update.insert_row_order(&row_order, &row_position);
      });
    let index = self
      .body
      .index_of_row(&txn, view_id, &row_order.id)
      .unwrap_or_default();
    Ok((index, row_order))
  }

  /// Remove the row
  /// The [RowOrder] of each view representing this row will be removed.
  pub async fn remove_row(&mut self, row_id: &RowId) {
    {
      let mut txn = self.collab.transact_mut();
      self.body.views.update_all_views(&mut txn, |_, update| {
        update.remove_row_order(row_id);
      });
    };
  }

  pub async fn move_row(&mut self, from_row_id: &str, to_row_id: &str) {
    let mut txn = self.collab.transact_mut();
    self.body.views.update_all_views(&mut txn, |_, update| {
      update.move_row_order(from_row_id, to_row_id);
    });
  }

  pub async fn remove_rows(&mut self, row_ids: &[RowId]) {
    {
      let mut txn = self.collab.transact_mut();
      self.body.views.update_all_views(&mut txn, |_, mut update| {
        for row_id in row_ids {
          update = update.remove_row_order(row_id);
        }
      });
    };
  }

  /// Update the row
  pub async fn update_row<F>(&mut self, row_id: RowId, f: F)
  where
    F: FnOnce(RowUpdate),
  {
    self.body.block.update_row(row_id, f).await;
  }

  /// Update the meta of the row
  pub async fn update_row_meta<F>(&mut self, row_id: &RowId, f: F)
  where
    F: FnOnce(RowMetaUpdate),
  {
    self.body.block.update_row_meta(row_id, f).await;
  }

  /// Return the index of the row in the given view.
  /// Return None if the row is not found.
  pub fn index_of_row(&self, view_id: &str, row_id: &RowId) -> Option<usize> {
    let txn = self.collab.transact();
    self.body.index_of_row(&txn, view_id, row_id)
  }

  /// Return the [Row] with the given row id.
  pub async fn get_row(&self, row_id: &RowId) -> Row {
    let row = self.body.block.get_database_row(row_id).await;
    match row {
      None => Row::empty(row_id.clone(), &self.get_database_id()),
      Some(row) => row
        .read()
        .await
        .get_row()
        .unwrap_or_else(|| Row::empty(row_id.clone(), &self.get_database_id())),
    }
  }

  /// Return the [RowMeta] with the given row id.
  pub async fn get_row_meta(&self, row_id: &RowId) -> Option<RowMeta> {
    self.body.block.get_row_meta(row_id).await
  }

  /// Return [TypeOptionCellReader] for the given field id.
  pub fn get_cell_reader(&self, field_id: &str) -> Option<Box<dyn TypeOptionCellReader>> {
    let txn = self.collab.transact();
    let field = self.body.fields.get_field(&txn, field_id)?;
    drop(txn);

    let field_type = FieldType::from(field.field_type);
    let type_option = field.get_any_type_option(field_type.type_id())?;
    Some(type_option_cell_reader(type_option, &field_type))
  }

  /// Return [TypeOptionCellWriter] for the given field id.
  pub fn get_cell_writer(&self, field_id: &str) -> Option<Box<dyn TypeOptionCellWriter>> {
    let txn = self.collab.transact();
    let field = self.body.fields.get_field(&txn, field_id)?;
    drop(txn);

    let field_type = FieldType::from(field.field_type);
    let type_option = field.get_any_type_option(field_type.type_id())?;
    Some(type_option_cell_writer(type_option, &field_type))
  }

  #[instrument(level = "debug", skip_all)]
  pub fn init_database_row(&self, row_id: &RowId, ret: Option<InitRowChan>) {
    self.body.block.init_database_row(row_id, ret)
  }

  pub fn init_database_rows<'a, T: Into<RowId> + Send + 'a>(
    &'a self,
    row_ids: Vec<T>,
    chunk_size: usize,
    cancel_token: Option<CancellationToken>,
    auto_fetch: bool,
  ) -> impl Stream<Item = Result<Arc<RwLock<DatabaseRow>>, DatabaseError>> + 'a {
    let row_ids_chunk_stream = stream::iter(
      row_ids
        .into_iter()
        .map(Into::into)
        .collect::<Vec<RowId>>()
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<RowId>>>(),
    );

    // Stream that processes each chunk and yields rows, checking for cancellation if the token is provided
    row_ids_chunk_stream
      .then(move |chunk| {
        let cancel_token = cancel_token.clone();
        async move {
          if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
              return Err(DatabaseError::ActionCancelled);
            }
          }

          self.body.block.init_database_rows(chunk, auto_fetch).await
        }
      })
      .filter_map(|result| async {
        match result {
          Ok(rows) => Some(stream::iter(rows.into_iter().map(Ok))),
          Err(err) => {
            error!("Error initializing database rows: {:?}", err);
            None
          },
        }
      })
      .flatten()
  }

  /// Return None if the row is not initialized.
  /// Use [Self::get_or_init_database_row] to initialize the row.
  pub async fn get_database_row(&self, row_id: &RowId) -> Option<Arc<RwLock<DatabaseRow>>> {
    self.body.block.get_database_row(row_id).await
  }

  #[instrument(level = "debug", skip_all)]
  pub async fn get_row_detail(&self, row_id: &RowId) -> Option<RowDetail> {
    let database_row = self
      .body
      .block
      .get_or_init_database_row(row_id)
      .await
      .ok()?;
    let read_guard = database_row.read().await;
    read_guard.get_row_detail()
  }

  pub fn get_row_document_id(&self, row_id: &RowId) -> Option<String> {
    self.body.block.get_row_document_id(row_id)
  }

  /// Return a list of [Row] for the given view.
  /// The rows here are ordered by [RowOrder]s of the view.
  pub async fn get_rows_for_view(
    &self,
    view_id: &str,
    chunk_size: usize,
    cancel_token: Option<CancellationToken>,
    auto_fetch: bool,
  ) -> impl Stream<Item = Result<Row, DatabaseError>> + '_ {
    let row_orders = self.get_row_orders_for_view(view_id);
    self
      .get_rows_from_row_orders(row_orders, chunk_size, cancel_token, auto_fetch)
      .await
  }

  pub async fn get_row_order_at_index(&self, view_id: &str, index: u32) -> Option<RowOrder> {
    let txn = self.collab.transact();
    self.body.views.get_row_order_at_index(&txn, view_id, index)
  }

  pub fn get_row_orders_for_view(&self, view_id: &str) -> Vec<RowOrder> {
    let txn = self.collab.transact();
    self.body.views.get_row_orders(&txn, view_id)
  }

  pub fn get_row_index(&self, view_id: &str, row_id: &RowId) -> Option<usize> {
    let txn = self.collab.transact();
    self.body.index_of_row(&txn, view_id, row_id)
  }

  /// Return a list of [Row] for the given view.
  /// The rows here is ordered by the [RowOrder] of the view.
  pub async fn get_rows_from_row_orders(
    &self,
    row_orders: Vec<RowOrder>,
    chunk_size: usize,
    cancel_token: Option<CancellationToken>,
    auto_fetch: bool,
  ) -> impl Stream<Item = Result<Row, DatabaseError>> + '_ {
    let row_ids = row_orders.iter().map(|order| order.id.clone()).collect();
    let rows_stream = self.init_database_rows(row_ids, chunk_size, cancel_token, auto_fetch);
    let database_id = self.get_database_id();
    rows_stream.then(move |result| {
      let database_id = database_id.clone();
      async move {
        let row = result?;
        let read_guard = row.read().await;
        let row_id = read_guard.row_id.clone();
        let row = read_guard
          .get_row()
          .unwrap_or_else(|| Row::empty(row_id, &database_id));
        Ok(row)
      }
    })
  }

  /// Return a list of [RowCell] for the given view and field.
  pub async fn get_cells_for_field(
    &self,
    view_id: &str,
    field_id: &str,
    auto_fetch: bool,
  ) -> Vec<RowCell> {
    let txn = self.collab.transact();
    self
      .body
      .get_cells_for_field(&txn, view_id, field_id, auto_fetch)
      .await
  }

  /// Return the [RowCell] with the given row id and field id.
  pub async fn get_cell(&self, field_id: &str, row_id: &RowId) -> RowCell {
    let cell = self.body.block.get_cell(row_id, field_id).await;
    RowCell::new(row_id.clone(), cell)
  }

  pub fn index_of_field(&self, view_id: &str, field_id: &str) -> Option<usize> {
    let txn = self.collab.transact();
    self.body.index_of_field(&txn, view_id, field_id)
  }

  /// Returns the [Field] with the given field ids.
  /// The fields are unordered.
  pub fn get_fields(&self, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.collab.transact();
    self.body.fields.get_fields_with_txn(&txn, field_ids)
  }

  /// Get all fields in the database
  /// These fields are ordered by the [FieldOrder] of the view
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return the fields with the given ids
  pub fn get_fields_in_view(&self, view_id: &str, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.collab.transact();
    self.body.get_fields_in_view(&txn, view_id, field_ids)
  }

  /// Creates a new field, inserts field order and adds a field setting. See
  /// `create_field_with_txn` for more information.
  pub fn create_field(
    &mut self,
    view_id: Option<&str>,
    field: Field,
    position: &OrderObjectPosition,
    field_settings_by_layout: HashMap<DatabaseLayout, FieldSettingsMap>,
  ) {
    let mut txn = self.collab.transact_mut();
    self.body.create_field(
      &mut txn,
      view_id,
      field,
      position,
      &field_settings_by_layout,
    );
  }

  pub fn create_field_with_mut(
    &mut self,
    view_id: &str,
    name: String,
    field_type: i64,
    position: &OrderObjectPosition,
    f: impl FnOnce(&mut Field),
    field_settings_by_layout: HashMap<DatabaseLayout, FieldSettingsMap>,
  ) -> (usize, Field) {
    let mut field = Field::new(gen_field_id(), name, field_type, false);
    f(&mut field);
    let mut txn = self.collab.transact_mut();
    self.body.create_field(
      &mut txn,
      Some(view_id),
      field.clone(),
      position,
      &field_settings_by_layout,
    );
    let index = self
      .body
      .index_of_field(&txn, view_id, &field.id)
      .unwrap_or_default();

    (index, field)
  }

  pub fn delete_field(&mut self, field_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_all_views(&mut txn, |_view_id, update| {
        update
          .remove_field_order(field_id)
          .remove_field_setting(field_id);
      });
    self.body.fields.delete_field(&mut txn, field_id);
  }

  pub fn get_all_group_setting<T: TryFrom<GroupSettingMap>>(&self, view_id: &str) -> Vec<T> {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_view_group_setting(&txn, view_id)
      .into_iter()
      .flat_map(|setting| T::try_from(setting).ok())
      .collect()
  }

  /// Add a group setting to the view. If the setting already exists, it will be replaced.
  pub fn insert_group_setting(&mut self, view_id: &str, group_setting: impl Into<GroupSettingMap>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_groups(|txn, group_update| {
          let group_setting = group_setting.into();
          let settings = if let Some(Any::String(setting_id)) = group_setting.get("id") {
            group_update.upsert(txn, setting_id)
          } else {
            group_update.push_back(txn, MapPrelim::default())
          };
          Any::from(group_setting).fill(txn, &settings).unwrap();
        });
      });
  }

  pub fn delete_group_setting(&mut self, view_id: &str, group_setting_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_groups(|txn, group_update| {
          if let Some(i) = group_update.index_by_id(txn, group_setting_id) {
            group_update.remove(txn, i);
          }
        });
      });
  }

  pub fn update_group_setting(
    &mut self,
    view_id: &str,
    setting_id: &str,
    f: impl FnOnce(&mut GroupSettingMap),
  ) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |view_update| {
        view_update.update_groups(|txn, group_update| {
          group_update.update_map(txn, setting_id, f);
        });
      });
  }

  pub fn remove_group_setting(&mut self, view_id: &str, setting_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_groups(|txn, group_update| {
          if let Some(i) = group_update.index_by_id(txn, setting_id) {
            group_update.remove(txn, i);
          }
        });
      });
  }

  pub fn insert_sort(&mut self, view_id: &str, sort: impl Into<SortMap>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_sorts(|txn, sort_update| {
          let sort = sort.into();
          if let Some(Any::String(sort_id)) = sort.get("id") {
            let map_ref: MapRef = sort_update.upsert(txn, sort_id);
            Any::from(sort).fill(txn, &map_ref).unwrap();
          } else {
            sort_update.push_back(txn, sort);
          }
        });
      });
  }

  pub fn move_sort(&mut self, view_id: &str, from_sort_id: &str, to_sort_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_sorts(|txn, sort_update| {
          if let Some(from) = sort_update.index_by_id(txn, from_sort_id) {
            if let Some(to) = sort_update.index_by_id(txn, to_sort_id) {
              sort_update.move_to(txn, from, to);
            }
          }
        });
      });
  }

  pub fn get_all_sorts<T>(&self, view_id: &str) -> Vec<T>
  where
    T: TryFrom<SortMap>,
    <T as TryFrom<SortMap>>::Error: Debug,
  {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_view_sorts(&txn, view_id)
      .into_iter()
      .flat_map(|sort| match T::try_from(sort) {
        Ok(sort) => Some(sort),
        Err(err) => {
          error!("Failed to convert sort, error: {:?}", err);
          None
        },
      })
      .collect()
  }

  pub fn get_sort<T>(&self, view_id: &str, sort_id: &str) -> Option<T>
  where
    T: TryFrom<SortMap>,
    <T as TryFrom<SortMap>>::Error: Debug,
  {
    let sort_id: Any = sort_id.into();
    let txn = self.collab.transact();
    let mut sorts = self
      .body
      .views
      .get_view_sorts(&txn, view_id)
      .into_iter()
      .filter(|sort_map| sort_map.get("id") == Some(&sort_id))
      .flat_map(|value| match T::try_from(value) {
        Ok(sort) => Some(sort),
        Err(err) => {
          error!("Failed to convert sort, error: {:?}", err);
          None
        },
      })
      .collect::<Vec<T>>();
    if sorts.is_empty() {
      None
    } else {
      Some(sorts.remove(0))
    }
  }

  pub fn remove_sort(&mut self, view_id: &str, sort_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_sorts(|txn, sort_update| {
          if let Some(i) = sort_update.index_by_id(txn, sort_id) {
            sort_update.remove(txn, i);
          }
        });
      });
  }

  pub fn remove_all_sorts(&mut self, view_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_sorts(|txn, sort_update| {
          sort_update.clear(txn);
        });
      });
  }

  pub fn get_all_calculations<T: TryFrom<CalculationMap>>(&self, view_id: &str) -> Vec<T> {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_view_calculations(&txn, view_id)
      .into_iter()
      .flat_map(|calculation| T::try_from(calculation).ok())
      .collect()
  }

  pub fn get_calculation<T: TryFrom<CalculationMap>>(
    &self,
    view_id: &str,
    field_id: &str,
  ) -> Option<T> {
    let field_id: Any = field_id.into();
    let txn = self.collab.transact();
    let mut calculations = self
      .body
      .views
      .get_view_calculations(&txn, view_id)
      .into_iter()
      .filter(|calculations_map| calculations_map.get("field_id") == Some(&field_id))
      .flat_map(|value| T::try_from(value).ok())
      .collect::<Vec<T>>();

    if calculations.is_empty() {
      None
    } else {
      Some(calculations.remove(0))
    }
  }

  pub fn update_calculation(&mut self, view_id: &str, calculation: impl Into<CalculationMap>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_calculations(|txn, calculation_update| {
          let calculation = calculation.into();
          if let Some(Any::String(calculation_id)) = calculation.get("id") {
            let map_ref: MapRef = calculation_update.upsert(txn, calculation_id);
            Any::from(calculation).fill(txn, &map_ref).unwrap();
          }
        });
      });
  }

  pub fn remove_calculation(&mut self, view_id: &str, calculation_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_calculations(|txn, calculation_update| {
          if let Some(i) = calculation_update.index_by_id(txn, calculation_id) {
            calculation_update.remove(txn, i);
          }
        });
      });
  }

  pub fn get_all_filters<T>(&self, view_id: &str) -> Vec<T>
  where
    T: TryFrom<FilterMap>,
    <T as TryFrom<FilterMap>>::Error: Debug,
  {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_view_filters(&txn, view_id)
      .into_iter()
      .flat_map(|setting| match T::try_from(setting) {
        Ok(filter) => Some(filter),
        Err(err) => {
          error!("Failed to convert filter: {:?}", err);
          None
        },
      })
      .collect()
  }

  pub fn get_filter<T>(&self, view_id: &str, filter_id: &str) -> Option<T>
  where
    T: TryFrom<FilterMap>,
    <T as TryFrom<FilterMap>>::Error: Debug,
  {
    let filter_id: Any = filter_id.into();
    let txn = self.collab.transact();
    let mut filters = self
      .body
      .views
      .get_view_filters(&txn, view_id)
      .into_iter()
      .filter(|filter_map| filter_map.get("id") == Some(&filter_id))
      .flat_map(|value| match T::try_from(value) {
        Ok(filter) => Some(filter),
        Err(err) => {
          error!("Failed to convert filter, error: {:?}", err);
          None
        },
      })
      .collect::<Vec<T>>();
    if filters.is_empty() {
      None
    } else {
      Some(filters.remove(0))
    }
  }

  pub fn update_filter(&mut self, view_id: &str, filter_id: &str, f: impl FnOnce(&mut FilterMap)) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |view_update| {
        view_update.update_filters(|txn, filter_update| {
          let map: MapRef = filter_update.upsert(txn, filter_id);
          let mut filter_map = map.to_json(txn).into_map().unwrap();
          f(&mut filter_map);
          Any::from(filter_map).fill(txn, &map).unwrap();
        });
      });
  }

  pub fn remove_filter(&mut self, view_id: &str, filter_id: &str) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_filters(|txn, filter_update| {
          if let Some(i) = filter_update.index_by_id(txn, filter_id) {
            filter_update.remove(txn, i);
          }
        });
      });
  }

  /// Add a filter to the view. If the setting already exists, it will be replaced.
  pub fn insert_filter(&mut self, view_id: &str, filter: impl Into<FilterMap>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_filters(|txn, filter_update| {
          let filter = filter.into();
          if let Some(Any::String(filter_id)) = filter.get("id") {
            let map_ref: MapRef = filter_update.upsert(txn, filter_id);
            Any::from(filter).fill(txn, &map_ref).unwrap();
          } else {
            let map_ref = filter_update.push_back(txn, MapPrelim::default());
            Any::from(filter).fill(txn, &map_ref).unwrap();
          }
        });
      });
  }

  /// Sets the filters of a database view. Requires two generics to work around the situation where
  /// `Into<AnyMap>` is only implemented for `&T`, not `T` itself. (alternatively, `From<&T>` is
  /// implemented for `AnyMap`, but not `From<T>`).
  ///
  /// * `T`: needs to be able to do `AnyMap::from(&T)`.
  /// * `U`: needs to implement `Into<AnyMap>`, could be just an identity conversion.
  pub fn save_filters<T, U>(&mut self, view_id: &str, filters: &[T])
  where
    U: for<'a> From<&'a T> + Into<FilterMap>,
  {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.set_filters(
          filters
            .iter()
            .map(|filter| U::from(filter))
            .map(Into::into)
            .collect(),
        );
      });
  }

  pub fn get_layout_setting<T: From<LayoutSetting>>(
    &self,
    view_id: &str,
    layout_ty: &DatabaseLayout,
  ) -> Option<T> {
    let txn = self.collab.transact();
    self.body.views.get_layout_setting(&txn, view_id, layout_ty)
  }

  pub fn insert_layout_setting<T: Into<LayoutSetting>>(
    &mut self,
    view_id: &str,
    layout_ty: &DatabaseLayout,
    layout_setting: T,
  ) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
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
    let txn = self.collab.transact();
    let mut field_settings_map = self
      .body
      .views
      .get_view_field_settings(&txn, view_id)
      .into_inner()
      .into_iter()
      .map(|(field_id, field_setting)| (field_id, T::from(field_setting)))
      .collect::<HashMap<String, T>>();

    if let Some(field_ids) = field_ids {
      field_settings_map.retain(|field_id, _| field_ids.contains(field_id));
    }

    field_settings_map
  }

  pub fn set_field_settings(
    &mut self,
    view_id: &str,
    field_settings_map: FieldSettingsByFieldIdMap,
  ) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.set_field_settings(field_settings_map);
      })
  }

  pub fn update_field_settings(
    &mut self,
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

    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        let field_settings = field_settings.into();
        update.update_field_settings_for_fields(
          field_ids,
          |txn, field_setting_update, field_id, _layout_ty| {
            let map_ref: MapRef = field_setting_update.get_or_init(txn, field_id);
            Any::from(field_settings.clone())
              .fill(txn, &map_ref)
              .unwrap();
          },
        );
      })
  }

  pub fn remove_field_settings_for_fields(&mut self, view_id: &str, field_ids: Vec<String>) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.update_field_settings_for_fields(
          field_ids,
          |txn, field_setting_update, field_id, _layout_ty| {
            field_setting_update.remove(txn, field_id);
          },
        );
      })
  }

  /// Update the layout type of the view.
  pub fn update_layout_type(&mut self, view_id: &str, layout_type: &DatabaseLayout) {
    let mut txn = self.collab.transact_mut();
    self
      .body
      .views
      .update_database_view(&mut txn, view_id, |update| {
        update.set_layout_type(*layout_type);
      });
  }

  /// Returns all the views that the current database has.
  // TODO (RS): Implement the creation of a default view when fetching all database views returns an empty result, with the exception of inline views.
  pub fn get_all_database_views_meta(&self) -> Vec<DatabaseViewMeta> {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_all_views_meta(&txn)
      .into_iter()
      .filter(|view| !view.is_inline)
      .collect()
  }

  /// Create a linked view to existing database
  pub fn create_linked_view(&mut self, params: CreateViewParams) -> Result<(), DatabaseError> {
    let mut txn = self.collab.transact_mut();
    let inline_view_id = self.body.get_inline_view_id(&txn);
    let row_orders = self.body.views.get_row_orders(&txn, &inline_view_id);
    let field_orders = self.body.views.get_field_orders(&txn, &inline_view_id);
    trace!(
      "Create linked view:{} rows:{}, fields:{}",
      params.view_id,
      row_orders.len(),
      field_orders.len()
    );

    self
      .body
      .create_linked_view(&mut txn, params, field_orders, row_orders)?;
    Ok(())
  }

  /// Create a linked view that duplicate the target view's setting including filter, sort,
  /// group, field setting, etc.
  pub fn duplicate_linked_view(&mut self, view_id: &str) -> Option<DatabaseView> {
    let mut txn = self.collab.transact_mut();
    let view = self.body.views.get_view(&txn, view_id)?;
    let timestamp = timestamp();
    let duplicated_view = DatabaseView {
      id: gen_database_view_id(),
      name: format!("{}-copy", view.name),
      created_at: timestamp,
      modified_at: timestamp,
      ..view
    };
    self
      .body
      .views
      .insert_view(&mut txn, duplicated_view.clone());

    Some(duplicated_view)
  }

  /// Duplicate the row, and insert it after the original row.
  pub async fn duplicate_row(&self, row_id: &RowId) -> Option<CreateRowParams> {
    let database_id = self.get_database_id();
    let row = self
      .body
      .block
      .get_database_row(row_id)
      .await?
      .read()
      .await
      .get_row()?;
    let timestamp = timestamp();
    Some(CreateRowParams {
      id: gen_row_id(),
      database_id,
      cells: row.cells,
      height: row.height,
      visibility: row.visibility,
      row_position: OrderObjectPosition::After(row.id.into()),
      created_at: timestamp,
      modified_at: timestamp,
    })
  }

  pub fn duplicate_field(
    &mut self,
    view_id: &str,
    field_id: &str,
    f: impl FnOnce(&Field) -> String,
  ) -> Option<(usize, Field)> {
    let mut txn = self.collab.transact_mut();
    if let Some(mut field) = self.body.fields.get_field(&txn, field_id) {
      field.id = gen_field_id();
      field.name = f(&field);
      self.body.insert_field(&mut txn, field.clone(), field_id);
      let index = self
        .body
        .index_of_field(&txn, view_id, &field.id)
        .unwrap_or_default();
      Some((index, field))
    } else {
      None
    }
  }

  pub fn get_primary_field(&self) -> Option<Field> {
    let txn = self.collab.transact();
    self.body.fields.get_primary_field(&txn)
  }

  /// Return all fields
  /// Use [Database::get_fields_in_view] If you want to ordered fields for specific view
  pub fn get_all_fields(&self) -> Vec<Field> {
    let txn = self.collab.transact();
    self.body.fields.get_all_fields(&txn)
  }

  pub async fn get_database_data(&self, chunk_size: usize, auto_fetch: bool) -> DatabaseData {
    let txn = self.collab.transact();

    let database_id = self.body.get_database_id(&txn);
    let inline_view_id = self.body.get_inline_view_id(&txn);
    let views = self.get_all_views();
    let fields = self.body.get_fields_in_view(&txn, &inline_view_id, None);
    let rows_stream = self.get_all_rows(chunk_size, None, auto_fetch).await;
    let rows: Vec<Row> = rows_stream
      .filter_map(|result| async move { result.ok() })
      .collect()
      .await;

    DatabaseData {
      database_id,
      fields,
      rows,
      views,
    }
  }

  pub fn get_view(&self, view_id: &str) -> Option<DatabaseView> {
    let txn = self.collab.transact();
    self.body.views.get_view(&txn, view_id)
  }

  pub async fn to_json_value(&self) -> JsonValue {
    let database_data = self.get_database_data(20, false).await;
    serde_json::to_value(&database_data).unwrap()
  }

  pub async fn get_all_rows(
    &self,
    chunk_size: usize,
    cancel_token: Option<CancellationToken>,
    auto_fetch: bool,
  ) -> impl Stream<Item = Result<Row, DatabaseError>> + '_ {
    let row_orders = {
      let txn = self.collab.transact();
      let inline_view_id = self.body.get_inline_view_id(&txn);
      self.body.views.get_row_orders(&txn, &inline_view_id)
    };

    self
      .get_rows_from_row_orders(row_orders, chunk_size, cancel_token, auto_fetch)
      .await
  }

  pub async fn collect_all_rows(&self, auto_fetch: bool) -> Vec<Result<Row, DatabaseError>> {
    let rows_stream = self.get_all_rows(20, None, auto_fetch).await;
    rows_stream.collect::<Vec<_>>().await
  }

  /// Return row orders of the inline database view
  pub async fn get_all_row_orders(&self) -> Vec<RowOrder> {
    let txn = self.collab.transact();
    let inline_view_id = self.body.get_inline_view_id(&txn);
    self.body.views.get_row_orders(&txn, &inline_view_id)
  }

  /// The inline view is the view that create with the database when initializing
  pub fn get_first_database_view_id(&self) -> Option<String> {
    let txn = self.collab.transact();
    self
      .body
      .views
      .get_all_views(&txn)
      .first()
      .map(|result| result.id.clone())
  }

  /// Delete a view from the database. If the view is the inline view it will clear all
  /// the linked views as well. Otherwise, just delete the view with given view id.
  pub fn delete_view(&mut self, view_id: &str) -> Vec<String> {
    let mut txn = self.collab.transact_mut();
    if self.body.get_inline_view_id(&txn) == view_id {
      let views = self.body.views.get_all_views_meta(&txn);
      self.body.views.clear(&mut txn);
      views.into_iter().map(|view| view.id).collect()
    } else {
      self.body.views.delete_view(&mut txn, view_id);
      vec![view_id.to_string()]
    }
  }

  pub fn get_field(&self, field_id: &str) -> Option<Field> {
    let txn = self.collab.transact();
    self.body.fields.get_field(&txn, field_id)
  }

  pub fn insert_field(&mut self, field: Field) {
    let mut txn = self.collab.transact_mut();
    self.body.fields.insert_field(&mut txn, field);
  }

  pub fn update_field<F>(&mut self, field_id: &str, f: F)
  where
    F: FnOnce(FieldUpdate),
  {
    let mut txn = self.collab.transact_mut();
    self.body.fields.update_field(&mut txn, field_id, f);
  }
}

impl Deref for Database {
  type Target = Collab;

  fn deref(&self) -> &Self::Target {
    &self.collab
  }
}

impl DerefMut for Database {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.collab
  }
}

impl Borrow<Collab> for Database {
  #[inline]
  fn borrow(&self) -> &Collab {
    &self.collab
  }
}

impl BorrowMut<Collab> for Database {
  fn borrow_mut(&mut self) -> &mut Collab {
    &mut self.collab
  }
}

pub fn gen_database_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

pub fn gen_database_view_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

pub fn gen_field_id() -> String {
  nanoid!(6)
}
pub fn gen_database_file_id() -> String {
  uuid::Uuid::new_v4().to_string()
}

pub fn gen_row_id() -> RowId {
  RowId::from(uuid::Uuid::new_v4().to_string())
}

pub fn get_row_document_id(row_id: &RowId) -> Result<String, DatabaseError> {
  let row_id = Uuid::parse_str(row_id)
    .map_err(|_err| DatabaseError::InvalidRowID("Failed to parse row id"))?;
  Ok(meta_id_from_row_id(&row_id, RowMetaKey::DocumentId))
}

pub fn gen_database_calculation_id() -> String {
  nanoid!(6)
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
/// It's used when duplicating a database, or during import and export.
#[derive(Clone, Serialize, Deserialize)]
pub struct DatabaseData {
  pub database_id: String,
  pub views: Vec<DatabaseView>,
  pub fields: Vec<Field>,
  pub rows: Vec<Row>,
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

pub fn get_database_row_ids(collab: &Collab) -> Option<Vec<String>> {
  let txn = collab.context.transact();
  let views: MapRef = collab.data.get_with_path(&txn, [DATABASE, VIEWS])?;
  let metas: MapRef = collab
    .data
    .get_with_path(&txn, [DATABASE, DATABASE_METAS])?;

  let views = DatabaseViews::new(CollabOrigin::Empty, views, None);
  let meta = MetaMap::new(metas);

  let inline_view_id = meta.get_inline_view_id(&txn)?;
  Some(
    views
      .get_row_orders(&txn, &inline_view_id)
      .into_iter()
      .map(|order| order.id.to_string())
      .collect(),
  )
}

pub fn reset_inline_view_id<F>(collab: &mut Collab, f: F) -> Result<(), DatabaseError>
where
  F: FnOnce(String) -> String,
{
  let mut txn = collab.context.transact_mut();
  if let Some(container) = collab.data.get_with_path(&txn, [DATABASE, DATABASE_METAS]) {
    let map = MetaMap::new(container);
    let inline_view_id = map.get_inline_view_id(&txn).ok_or_else(|| {
      DatabaseError::NoRequiredData("Can not find the inline view id".to_string())
    })?;
    let new_inline_view_id = f(inline_view_id);
    map.set_inline_view_id(&mut txn, &new_inline_view_id);
    Ok(())
  } else {
    Err(DatabaseError::NoRequiredData(
      "Can not find the database metas".to_string(),
    ))
  }
}

pub fn mut_database_views_with_collab<F>(collab: &mut Collab, f: F)
where
  F: FnMut(&mut DatabaseView),
{
  let origin = collab.origin().clone();
  let mut txn = collab.context.transact_mut();

  if let Some(container) = collab
    .data
    .get_with_path::<_, _, MapRef>(&txn, [DATABASE, VIEWS])
  {
    let views = DatabaseViews::new(origin, container, None);
    let mut reset_views = views.get_all_views(&txn);

    reset_views.iter_mut().for_each(f);
    for view in reset_views {
      views.insert_view(&mut txn, view);
    }
  }
}

pub fn is_database_collab(collab: &Collab) -> bool {
  let txn = collab.transact();
  collab.get_with_txn(&txn, DATABASE).is_some()
}

/// Quickly retrieve the inline view ID of a database.
/// Use this function when instantiating a [Database] object is too resource-intensive,
/// and you need the inline view ID of a specific database.
pub fn get_inline_view_id(collab: &Collab) -> Option<String> {
  let txn = collab.context.transact();
  let metas: MapRef = collab
    .data
    .get_with_path(&txn, [DATABASE, DATABASE_METAS])?;
  let meta = MetaMap::new(metas);
  meta.get_inline_view_id(&txn)
}

/// Quickly retrieve database views meta.
/// Use this function when instantiating a [Database] object is too resource-intensive,
/// and you need the views meta of a specific database.
pub fn get_database_views_meta(collab: &Collab) -> Vec<DatabaseViewMeta> {
  let txn = collab.context.transact();
  let views: Option<MapRef> = collab.data.get_with_path(&txn, [DATABASE, VIEWS]);
  let views = DatabaseViews::new(CollabOrigin::Empty, views.unwrap(), None);
  views.get_all_views_meta(&txn)
}

pub struct DatabaseBody {
  pub root: MapRef,
  pub views: Arc<DatabaseViews>,
  pub fields: Arc<FieldMap>,
  pub metas: Arc<MetaMap>,
  /// It used to keep track of the blocks. Each block contains a list of [Row]s
  /// A database rows will be stored in multiple blocks.
  pub block: Block,
  pub notifier: Option<DatabaseNotify>,
}

impl DatabaseBody {
  pub fn open(collab: Collab, context: DatabaseContext) -> Result<(Self, Collab), DatabaseError> {
    CollabType::Database.validate_require_data(&collab)?;
    let body = Self::from_collab(
      &collab,
      context.database_row_collab_service,
      Some(context.notifier),
    )
    .ok_or_else(|| DatabaseError::NoRequiredData("Can not open database".to_string()))?;
    Ok((body, collab))
  }

  async fn create(
    mut collab: Collab,
    database_id: String,
    context: DatabaseContext,
    new_rows: Vec<CreateRowParams>,
    new_fields: Vec<Field>,
    new_views: Vec<CreateViewParams>,
  ) -> Result<(Self, Collab), DatabaseError> {
    let origin = collab.origin().clone();
    let mut txn = collab.context.transact_mut();
    let root: MapRef = collab.data.get_or_init(&mut txn, DATABASE);
    root.insert(&mut txn, DATABASE_ID, &*database_id);
    let fields: MapRef = root.get_or_init(&mut txn, FIELDS); // { DATABASE: { FIELDS: {:} } }
    let views: MapRef = root.get_or_init(&mut txn, VIEWS); // { DATABASE: { FIELDS: {:}, VIEWS: {:} } }
    let metas: MapRef = root.get_or_init(&mut txn, DATABASE_METAS); // { DATABASE: { FIELDS: {:},  VIEWS: {:}, METAS: {:} } }

    let fields = FieldMap::new(fields, Some(context.notifier.field_change_tx.clone()));
    let views = DatabaseViews::new(origin, views, Some(context.notifier.view_change_tx.clone()));
    let block = Block::new(
      database_id.clone(),
      context.database_row_collab_service.clone(),
      Some(context.notifier.row_change_tx.clone()),
    );

    let database_id_uuid = Uuid::parse_str(&database_id)
      .map_err(|_| DatabaseError::InvalidDatabaseID("database_id is not a valid UUID"))?;
    let inline_view_id = database_inline_view_id(&database_id_uuid);

    // create rows
    let row_orders = block
      .create_rows(
        new_rows,
        context.database_collab_service.database_client_id().await,
      )
      .await;

    // create field orders
    let field_orders: Vec<FieldOrder> = new_fields.iter().map(FieldOrder::from).collect();

    // Insert the given fields into the database
    for field in new_fields {
      fields.insert_field(&mut txn, field);
    }

    let mut inline_view = DatabaseView::new(
      database_id,
      inline_view_id.to_string(),
      "".to_string(),
      DatabaseLayout::Grid,
    );
    inline_view.is_inline = true;
    inline_view.row_orders.clone_from(&row_orders);
    inline_view.field_orders.clone_from(&field_orders);
    views.insert_view(&mut txn, inline_view);

    let metas = MetaMap::new(metas);
    metas.set_inline_view_id(&mut txn, &inline_view_id.to_string());
    drop(txn);

    let body = DatabaseBody {
      root,
      views: views.into(),
      fields: fields.into(),
      metas: metas.into(),
      block,
      notifier: Some(context.notifier),
    };

    let mut txn = collab.context.transact_mut();
    for new_view in new_views {
      body.create_linked_view(&mut txn, new_view, field_orders.clone(), row_orders.clone())?;
    }
    drop(txn);

    Ok((body, collab))
  }

  /// Creates a [DatabaseBody] body from the given [Collab] instance. If the required fields are not
  /// present, it will return `None`.
  ///
  /// There is no [DatabaseNotify] or persistence layer in [DatabaseBody] created by this method.
  pub fn from_collab(
    collab: &Collab,
    collab_service: Arc<dyn DatabaseRowCollabService>,
    notifier: Option<DatabaseNotify>,
  ) -> Option<Self> {
    let txn = collab.context.transact();
    let origin = if notifier.is_some() {
      collab.origin().clone()
    } else {
      CollabOrigin::Empty
    };
    let root: MapRef = collab.data.get_with_txn(&txn, DATABASE)?;
    let database_id = root.get_with_txn(&txn, DATABASE_ID)?;
    let fields: MapRef = root.get_with_txn(&txn, FIELDS)?; // { DATABASE: { FIELDS: {:} } }
    let views: MapRef = root.get_with_txn(&txn, VIEWS)?; // { DATABASE: { FIELDS: {:}, VIEWS: {:} } }
    let metas: MapRef = root.get_with_txn(&txn, DATABASE_METAS)?; // { DATABASE: { FIELDS: {:},  VIEWS: {:}, METAS: {:} } }

    let fields = FieldMap::new(fields, notifier.as_ref().map(|n| n.field_change_tx.clone()));
    let views = DatabaseViews::new(
      origin,
      views,
      notifier.as_ref().map(|n| n.view_change_tx.clone()),
    );
    let metas = MetaMap::new(metas);
    let block = Block::new(
      database_id,
      collab_service,
      notifier.as_ref().map(|n| n.row_change_tx.clone()),
    );
    Some(Self {
      root,
      views: views.into(),
      fields: fields.into(),
      metas: metas.into(),
      block,
      notifier,
    })
  }

  /// Return database id from the given [Collab] instance. If the required fields are not found,
  /// it will return None.
  pub fn database_id_from_collab(collab: &Collab) -> Option<String> {
    let txn = collab.context.transact();
    let root = collab.data.get(&txn, DATABASE)?.cast::<MapRef>().ok()?;
    root.get(&txn, DATABASE_ID)?.cast::<String>().ok()
  }

  #[allow(dead_code)]
  pub(crate) fn inline_view_id_from_collab(collab: &Collab) -> Option<String> {
    let txn = collab.context.transact();
    let root = collab.data.get(&txn, DATABASE)?.cast::<MapRef>().ok()?;
    let database_id = root.get(&txn, DATABASE_ID)?.cast::<String>().ok()?;
    let views_map: MapRef = root.get_with_txn(&txn, VIEWS)?;
    let metas_map: MapRef = root.get_with_txn(&txn, DATABASE_METAS)?;
    let metas = MetaMap::new(metas_map);
    let views = DatabaseViews::new(CollabOrigin::Empty, views_map, None);

    let mut inline_view_id = metas.get_inline_view_id(&txn);
    if inline_view_id.is_none() {
      error!(
        "Inline view id is not found in the database:{}",
        database_id,
      );
      let view_metas = views.get_all_views_meta(&txn);
      inline_view_id = view_metas.first().map(|view| view.id.clone());
      if view_metas.is_empty() {
        error!(
          "Can't find any database views when inline view id is empty. current root map:{}",
          root.to_json(&txn)
        );
      } else {
        info!(
          "Can't find default inline view id, using {} as inline view id",
          inline_view_id.as_ref().unwrap()
        );
      }
    }

    inline_view_id
  }

  pub fn get_database_id<T: ReadTxn>(&self, txn: &T) -> String {
    self.root.get_with_txn(txn, DATABASE_ID).unwrap()
  }

  /// Create a new row from the given view.
  /// This row will be inserted into corresponding [Block]. The [RowOrder] of this row will
  /// be inserted to each view.
  pub async fn create_row(
    &self,
    params: CreateRowParams,
    client_id: ClientID,
  ) -> Result<RowOrder, DatabaseError> {
    let row_order = self.block.create_new_row(params, client_id).await?;
    Ok(row_order)
  }

  pub fn index_of_row<T: ReadTxn>(&self, txn: &T, view_id: &str, row_id: &RowId) -> Option<usize> {
    let view = self.views.get_view(txn, view_id)?;
    view.row_orders.iter().position(|order| &order.id == row_id)
  }

  pub fn get_inline_view_id<T: ReadTxn>(&self, txn: &T) -> String {
    self.try_get_inline_view_id(txn).unwrap()
  }

  pub(crate) fn try_get_inline_view_id<T: ReadTxn>(&self, txn: &T) -> Option<String> {
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    let mut inline_view_id = self.metas.get_inline_view_id(txn);
    if inline_view_id.is_none() {
      error!(
        "Inline view id is not found in the database:{}",
        self.get_database_id(txn)
      );
      let view_metas = self.views.get_all_views_meta(txn);
      inline_view_id = view_metas.first().map(|view| view.id.clone());
      if view_metas.is_empty() {
        let root = self.root.to_json(txn);
        error!(
          "Can't find any database views when inline view id is empty. current root map:{}",
          root
        );
      } else {
        info!(
          "Can't find default inline view id, using {} as inline view id",
          inline_view_id.as_ref().unwrap()
        );
      }
    }

    inline_view_id
  }

  /// Return the index of the field in the given view.
  pub fn index_of_field<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_id: &str,
  ) -> Option<usize> {
    let view = self.views.get_view(txn, view_id)?;
    view
      .field_orders
      .iter()
      .position(|order| order.id == field_id)
  }

  /// Return list of [RowCell] for the given view and field.
  pub async fn get_cells_for_field<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_id: &str,
    auto_fetch: bool,
  ) -> Vec<RowCell> {
    let row_orders = self.views.get_row_orders(txn, view_id);
    let rows = self
      .block
      .get_rows_from_row_orders(&row_orders, auto_fetch)
      .await;
    rows
      .into_iter()
      .map(|row| RowCell::new(row.id, row.cells.get(field_id).cloned()))
      .collect()
  }
  /// Get all fields in the database
  /// These fields are ordered by the [FieldOrder] of the view
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return the fields with the given ids
  pub fn get_fields_in_view<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_ids: Option<Vec<String>>,
  ) -> Vec<Field> {
    let field_orders = self.views.get_field_orders(txn, view_id);
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

  /// Create a new field that is used by `create_field`, `create_field_with_mut`, and
  /// `create_linked_view`. In all the database views, insert the field order and add a field setting.
  /// Then, add the field to the field map.
  ///
  /// # Arguments
  ///
  /// - `txn`: Read-write transaction in which this field creation will be performed.
  /// - `view_id`: If specified, the field order will only be inserted according to `position` in that
  ///   specific view. For the others, the field order will be pushed back. If `None`, the field order will
  ///   be inserted according to `position` for all the views.
  /// - `field`: Field to be inserted.
  /// - `position`: The position of the new field in the field order array.
  /// - `field_settings_by_layout`: Helps to create the field settings for the field.
  pub fn create_field(
    &self,
    txn: &mut TransactionMut,
    view_id: Option<&str>,
    field: Field,
    position: &OrderObjectPosition,
    field_settings_by_layout: &HashMap<DatabaseLayout, FieldSettingsMap>,
  ) {
    self.views.update_all_views(txn, |id, update| {
      let update = match view_id {
        Some(view_id) if id == view_id => update.insert_field_order(&field, position),
        Some(_) => update.insert_field_order(&field, &OrderObjectPosition::default()),
        None => update.insert_field_order(&field, position),
      };

      update.update_field_settings_for_fields(
        vec![field.id.clone()],
        |txn, field_setting_update, field_id, layout_ty| {
          let map_ref: MapRef = field_setting_update.get_or_init_map(txn, field_id);
          if let Some(settings) = field_settings_by_layout.get(&layout_ty) {
            Any::from(settings.clone()).fill(txn, &map_ref).unwrap();
          }
        },
      );
    });
    self.fields.insert_field(txn, field);
  }

  /// Creates a new field, add a field setting, but inserts the field after a
  /// certain field_id
  fn insert_field(&self, txn: &mut TransactionMut, field: Field, prev_field_id: &str) {
    self.views.update_all_views(txn, |_view_id, update| {
      update.insert_field_order(
        &field,
        &OrderObjectPosition::After(prev_field_id.to_string()),
      );
    });
    self.fields.insert_field(txn, field);
  }

  /// Create a [DatabaseView] for the current database.
  pub fn create_view(
    &self,
    txn: &mut TransactionMut,
    params: CreateViewParams,
    field_orders: Vec<FieldOrder>,
    row_orders: Vec<RowOrder>,
  ) -> Result<(), DatabaseError> {
    let params = CreateViewParamsValidator::validate(params)?;
    let database_id = self.get_database_id(txn);
    let view = DatabaseView {
      id: params.view_id,
      database_id,
      name: params.name,
      layout: params.layout,
      layout_settings: params.layout_settings,
      filters: params.filters,
      group_settings: params.group_settings,
      sorts: params.sorts,
      field_settings: params.field_settings,
      row_orders,
      field_orders,
      created_at: params.created_at,
      modified_at: params.modified_at,
      is_inline: false,
    };

    trace!(
      "Insert linked view to database: {}, view: {}",
      view.database_id, view.id
    );
    self.views.insert_view(txn, view);
    Ok(())
  }

  pub fn create_linked_view(
    &self,
    txn: &mut TransactionMut,
    params: CreateViewParams,
    field_orders: Vec<FieldOrder>,
    row_orders: Vec<RowOrder>,
  ) -> Result<(), DatabaseError> {
    let mut params = CreateViewParamsValidator::validate(params)?;
    let (deps_fields, deps_field_settings) = params.take_deps_fields();

    self.create_view(txn, params, field_orders, row_orders)?;

    // After creating the view, we need to create the fields that are used in the view.
    if !deps_fields.is_empty() {
      tracing::trace!("create linked view with deps fields: {:?}", deps_fields);
      deps_fields
        .into_iter()
        .zip(deps_field_settings)
        .for_each(|(field, field_settings)| {
          self.create_field(
            txn,
            None,
            field,
            &OrderObjectPosition::default(),
            &field_settings,
          );
        });
    }
    Ok(())
  }
}

pub fn try_fixing_database(
  collab: &mut Collab,
  database_meta: DatabaseMeta,
) -> Result<(), DatabaseError> {
  // check if inline view id
  let inline_view_id = {
    let txn = collab.context.transact();
    if let Some(container) = collab.data.get_with_path(&txn, [DATABASE, DATABASE_METAS]) {
      let map = MetaMap::new(container);
      map.get_inline_view_id(&txn)
    } else {
      None
    }
  };

  info!("[Fix]: inline view id: {:?}", inline_view_id);
  if inline_view_id.is_none() {
    if let Some(default_inline_view) = database_meta.linked_views.first() {
      let mut txn = collab.context.transact_mut();
      if let Some(container) = collab.data.get_with_path(&txn, [DATABASE, DATABASE_METAS]) {
        let map = MetaMap::new(container);
        info!("[Fix]: set inline view id to {}", default_inline_view);
        map.set_inline_view_id(&mut txn, default_inline_view);
      }
    } else {
      info!("[Fix]: no default inline view id found");
    }
  }

  Ok(())
}

fn database_inline_view_id(database_id: &Uuid) -> Uuid {
  let key = "inline_view_id";
  Uuid::new_v5(database_id, key.as_bytes())
}
