use crate::block::{Blocks, CreateRowParams};
use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::id_gen::ID_GEN;
use crate::meta::MetaMap;
use crate::rows::{Row, RowCell, RowId, RowUpdate};
use crate::views::{
  CreateDatabaseParams, CreateViewParams, DatabaseLayout, DatabaseView, FieldOrder, FilterMap,
  GroupSettingMap, LayoutSetting, RowOrder, SortMap, ViewDescription, ViewMap,
};
use collab::core::any_map::AnyMapExtension;
use collab::preclude::{
  Collab, JsonValue, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;

pub struct Database {
  #[allow(dead_code)]
  inner: Collab,
  pub(crate) root: MapRefWrapper,
  pub views: Rc<ViewMap>,
  pub fields: Rc<FieldMap>,
  pub metas: Rc<MetaMap>,
  pub blocks: Blocks,
}

const DATABASE_ID: &str = "id";
const DATABASE: &str = "database";
const FIELDS: &str = "fields";
const VIEWS: &str = "views";
const METAS: &str = "metas";
const DATABASE_INLINE_VIEW: &str = "iid";

pub struct DatabaseContext {
  pub collab: Collab,
  pub blocks: Blocks,
}

impl Database {
  pub fn create_with_inline_view(
    database_id: &str,
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Self, DatabaseError> {
    let this = Self::get_or_create(database_id, context)?;
    let (rows, fields, params) = params.split();
    let row_orders = this.blocks.create_rows(rows);
    let field_orders = fields.iter().map(FieldOrder::from).collect();

    this.root.with_transact_mut(|txn| {
      this.set_inline_view_with_txn(txn, &params.view_id);
      for field in fields {
        this.fields.insert_field_with_txn(txn, field);
      }
      this.create_view_with_txn(txn, params, field_orders, row_orders);
    });
    Ok(this)
  }

  pub fn get_or_create(database_id: &str, context: DatabaseContext) -> Result<Self, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID);
    }
    let collab = context.collab;
    let (database, fields, views, metas) = collab.with_transact_mut(|txn| {
      // { DATABASE: {:} }
      let database = collab
        .get_map_with_txn(txn, vec![DATABASE])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, DATABASE));

      database.insert_str_with_txn(txn, DATABASE_ID, database_id);

      // { DATABASE: { FIELDS: {:} } }
      let fields = collab
        .get_map_with_txn(txn, vec![DATABASE, FIELDS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, FIELDS));

      // { DATABASE: { FIELDS: {:}, VIEWS: {:} } }
      let views = collab
        .get_map_with_txn(txn, vec![DATABASE, VIEWS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, VIEWS));

      // { DATABASE: { FIELDS: {:},  VIEWS: {:}, METAS: {:} } }
      let metas = collab
        .get_map_with_txn(txn, vec![DATABASE, METAS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, METAS));

      (database, fields, views, metas)
    });
    let views = ViewMap::new(views);
    let fields = FieldMap::new(fields);
    let metas = MetaMap::new(metas);

    Ok(Self {
      inner: collab,
      root: database,
      blocks: context.blocks,
      views: Rc::new(views),
      fields: Rc::new(fields),
      metas: Rc::new(metas),
    })
  }

  pub fn get_database_id(&self) -> String {
    let txn = self.root.transact();
    // It's safe to unwrap. Because the database_id must exist
    self.root.get_str_with_txn(&txn, DATABASE_ID).unwrap()
  }

  pub fn get_database_id_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    self.root.get_str_with_txn(txn, DATABASE_ID).unwrap()
  }

  pub fn push_row(&self, params: CreateRowParams) -> Option<RowOrder> {
    let row_order = self.blocks.create_row(params)?;
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.push_row_order(&row_order);
      });
    });
    Some(row_order)
  }

  pub fn create_row(&self, view_id: &str, params: CreateRowParams) -> Option<(usize, RowOrder)> {
    self
      .root
      .with_transact_mut(|txn| self.create_row_with_txn(txn, view_id, params))
  }

  pub fn create_row_with_txn(
    &self,
    txn: &mut TransactionMut,
    view_id: &str,
    params: CreateRowParams,
  ) -> Option<(usize, RowOrder)> {
    let prev_row_id = params.prev_row_id.map(|value| value.to_string());
    if let Some(row_order) = self.blocks.create_row(params) {
      self.views.update_all_views_with_txn(txn, |update| {
        update.insert_row_order(&row_order, prev_row_id.as_ref());
      });

      let index = self
        .index_of_row_with_txn(txn, view_id, row_order.id)
        .unwrap_or_default();
      Some((index, row_order))
    } else {
      None
    }
  }

  pub fn remove_row(&self, row_id: RowId) -> Option<Row> {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_row_order(&row_id.to_string());
      });
      let row = self.blocks.get_row(row_id);
      self.blocks.remove_row(row_id);
      row
    })
  }

  pub fn update_row<R, F>(&self, row_id: R, f: F)
  where
    F: FnOnce(RowUpdate),
    R: Into<RowId>,
  {
    self.blocks.update_row(row_id, f);
  }

  pub fn index_of_row(&self, view_id: &str, row_id: RowId) -> Option<usize> {
    let view = self.views.get_view(view_id)?;
    view.row_orders.iter().position(|order| order.id == row_id)
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

  pub fn get_row<R>(&self, row_id: R) -> Option<Row>
  where
    R: Into<RowId>,
  {
    self.blocks.get_row(row_id.into())
  }

  pub fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let txn = self.root.transact();
    self.get_rows_for_view_with_txn(&txn, view_id)
  }

  pub fn get_rows_for_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<Row> {
    let row_orders = self.views.get_row_orders_with_txn(txn, view_id);
    self.blocks.get_rows_from_row_orders(&row_orders)
  }

  pub fn get_cells_for_field(&self, view_id: &str, field_id: &str) -> Vec<RowCell> {
    let txn = self.root.transact();
    self.get_cells_for_field_with_txn(&txn, view_id, field_id)
  }

  pub fn get_cell(&self, field_id: &str, row_id: RowId) -> Option<RowCell> {
    let cell = self.blocks.get_cell(field_id, row_id)?;
    Some(RowCell::new(row_id, cell))
  }

  pub fn get_cells_for_field_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_id: &str,
  ) -> Vec<RowCell> {
    let row_orders = self.views.get_row_orders_with_txn(txn, view_id);
    let rows = self.blocks.get_rows_from_row_orders(&row_orders);
    rows
      .into_iter()
      .flat_map(|row| match row.cells.get(field_id).cloned() {
        None => None,
        Some(cell) => Some(RowCell::new(row.id, cell)),
      })
      .collect::<Vec<RowCell>>()
  }

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

  /// Get all fields in the database
  /// If field_ids is None, return all fields
  /// If field_ids is Some, return the fields with the given ids
  pub fn get_fields(&self, view_id: &str, field_ids: Option<Vec<String>>) -> Vec<Field> {
    let txn = self.root.transact();
    self.get_fields_with_txn(&txn, view_id, field_ids)
  }

  pub fn get_fields_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    view_id: &str,
    field_ids: Option<Vec<String>>,
  ) -> Vec<Field> {
    let field_orders = self.views.get_field_orders_txn(txn, view_id);
    let mut all_field_map = self
      .fields
      .get_fields_with_txn(txn, field_ids)
      .into_iter()
      .map(|field| (field.id.clone(), field))
      .collect::<HashMap<String, Field>>();

    debug_assert!(field_orders.len() == all_field_map.len());

    field_orders
      .into_iter()
      .flat_map(|order| all_field_map.remove(&order.id))
      .collect()
  }

  pub fn push_field(&self, field: Field) {
    self.root.with_transact_mut(|txn| {
      self.push_field_with_txn(txn, field);
    })
  }

  pub fn push_field_with_txn(&self, txn: &mut TransactionMut, field: Field) {
    self.views.update_all_views_with_txn(txn, |update| {
      update.push_field_order(&field);
    });
    self.fields.insert_field_with_txn(txn, field);
  }

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
        update.remove_field_order(field_id);
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
    self.views.update_view(view_id, |update| {
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
    self.views.update_view(view_id, |update| {
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
    self.views.update_view(view_id, |view_update| {
      view_update.update_groups(|group_update| {
        group_update.update(setting_id, |mut map| {
          f(&mut map);
          map
        });
      });
    });
  }

  pub fn remove_group_setting(&self, view_id: &str, setting_id: &str) {
    self.views.update_view(view_id, |update| {
      update.update_groups(|group_update| {
        group_update.remove(setting_id);
      });
    });
  }

  pub fn insert_sort(&self, view_id: &str, sort: impl Into<SortMap>) {
    self.views.update_view(view_id, |update| {
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
    self.views.update_view(view_id, |update| {
      update.update_sorts(|sort_update| {
        sort_update.remove(sort_id);
      });
    });
  }

  pub fn remove_all_sorts(&self, view_id: &str) {
    self.views.update_view(view_id, |update| {
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
    self.views.update_view(view_id, |view_update| {
      view_update.update_filters(|filter_update| {
        filter_update.update(filter_id, |mut map| {
          f(&mut map);
          map
        });
      });
    });
  }

  pub fn remove_filter(&self, view_id: &str, filter_id: &str) {
    self.views.update_view(view_id, |update| {
      update.update_filters(|filter_update| {
        filter_update.remove(filter_id);
      });
    });
  }

  /// Add a group setting to the view. If the setting already exists, it will be replaced.
  pub fn insert_filter(&self, view_id: &str, filter: impl Into<FilterMap>) {
    self.views.update_view(view_id, |update| {
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

  pub fn insert_layout_setting<T: Into<LayoutSetting>>(
    &self,
    view_id: &str,
    layout_ty: &DatabaseLayout,
    layout_setting: T,
  ) {
    self.views.update_view(view_id, |update| {
      update.update_layout_settings(layout_ty, layout_setting.into());
    });
  }

  pub fn create_view(&self, params: CreateViewParams) {
    self.root.with_transact_mut(|txn| {
      let inline_view_id = self.get_inline_view_id_with_txn(txn);
      let row_orders = self.views.get_row_orders_with_txn(txn, &inline_view_id);
      let field_orders = self.views.get_field_orders_txn(txn, &inline_view_id);
      self.create_view_with_txn(txn, params, field_orders, row_orders);
    })
  }

  pub fn get_all_views_description(&self) -> Vec<ViewDescription> {
    let txn = self.root.transact();
    self.views.get_all_views_description_with_txn(&txn)
  }

  pub fn create_view_with_txn(
    &self,
    txn: &mut TransactionMut,
    params: CreateViewParams,
    field_orders: Vec<FieldOrder>,
    row_orders: Vec<RowOrder>,
  ) {
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
      row_orders,
      field_orders,
      created_at: timestamp,
      modified_at: timestamp,
    };
    self.views.insert_view_with_txn(txn, view);
  }

  pub fn get_view(&self, view_id: &str) -> Option<DatabaseView> {
    let txn = self.root.transact();
    self.views.get_view_with_txn(&txn, view_id)
  }

  /// Duplicate a view that shares the same rows as the original view.
  pub fn duplicate_view(&self, view_id: &str) -> Option<DatabaseView> {
    let view = self.views.get_view(view_id)?;
    let mut duplicated_view = view.clone();
    duplicated_view.id = gen_database_view_id();
    duplicated_view.created_at = timestamp();
    duplicated_view.modified_at = timestamp();
    duplicated_view.name = format!("{}-copy", view.name);
    self.views.insert_view(duplicated_view.clone());

    Some(duplicated_view)
  }

  /// Duplicate a row
  pub fn duplicate_row(&self, view_id: &str, row_id: RowId) -> Option<(usize, RowOrder)> {
    self.root.with_transact_mut(|txn| {
      if let Some(row) = self.blocks.get_row(row_id) {
        let params = CreateRowParams {
          id: gen_row_id(),
          cells: row.cells,
          height: row.height,
          visibility: row.visibility,
          prev_row_id: Some(row.id),
        };
        self.create_row_with_txn(txn, view_id, params)
      } else {
        None
      }
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

  pub fn duplicate_database_data(&self) -> DuplicatedDatabase {
    let inline_view_id = self.get_inline_view_id();
    let txn = self.root.transact();
    let mut view = self.views.get_view_with_txn(&txn, &inline_view_id).unwrap();
    let fields = self.get_fields_with_txn(&txn, &inline_view_id, None);
    let row_orders = self.views.get_row_orders_with_txn(&txn, &view.id);
    let rows = self
      .blocks
      .get_rows_from_row_orders(&row_orders)
      .into_iter()
      .map(|row| CreateRowParams {
        id: gen_row_id(),
        cells: row.cells,
        height: row.height,
        visibility: row.visibility,
        prev_row_id: None,
      })
      .collect::<Vec<CreateRowParams>>();

    view.id = gen_database_view_id();
    view.database_id = gen_database_id();
    DuplicatedDatabase { view, fields, rows }
  }

  pub fn create_default_field(
    &self,
    view_id: &str,
    name: String,
    field_type: i64,
    f: impl FnOnce(&mut Field),
  ) -> (usize, Field) {
    let mut field = Field::new(gen_field_id(), name, field_type, false);
    f(&mut field);
    let index = self.root.with_transact_mut(|txn| {
      self.push_field_with_txn(txn, field.clone());
      self
        .index_of_field_with_txn(txn, view_id, &field.id)
        .unwrap_or_default()
    });

    (index, field)
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
    let txn = self.inner.transact();
    self.get_database_rows_with_txn(&txn)
  }

  pub fn get_database_rows_with_txn<T: ReadTxn>(&self, txn: &T) -> Vec<Row> {
    let inline_view_id = self.get_inline_view_id_with_txn(txn);
    self.get_rows_for_view_with_txn(txn, &inline_view_id)
  }

  pub fn set_inline_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
    self
      .metas
      .insert_str_with_txn(txn, DATABASE_INLINE_VIEW, view_id);
  }

  /// The inline view is the view that create with the database when initializing
  pub fn get_inline_view_id(&self) -> String {
    let txn = self.root.transact();
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    self
      .metas
      .get_str_with_txn(&txn, DATABASE_INLINE_VIEW)
      .unwrap()
  }

  fn get_inline_view_id_with_txn<T: ReadTxn>(&self, txn: &T) -> String {
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    self
      .metas
      .get_str_with_txn(txn, DATABASE_INLINE_VIEW)
      .unwrap()
  }

  pub fn delete_view(&self, view_id: &str) {
    if self.is_inline_view(view_id) {
      self.root.with_transact_mut(|txn| {
        self.views.clear_with_txn(txn);
      });
    } else {
      self.root.with_transact_mut(|txn| {
        self.views.delete_view_with_txn(txn, view_id);
      });
    }
  }
}

pub fn gen_database_id() -> String {
  // nanoid calculator https://zelark.github.io/nano-id-cc/
  format!("d:{}", nanoid!(10))
}

pub fn gen_database_view_id() -> String {
  format!("d:{}", nanoid!(6))
}

pub fn gen_field_id() -> String {
  nanoid!(6)
}

pub fn gen_row_id() -> RowId {
  RowId::from(ID_GEN.lock().next_id())
}

pub fn gen_database_filter_id() -> String {
  nanoid!(6)
}

pub fn gen_database_group_id() -> String {
  nanoid!(6)
}

pub fn gen_database_sort_id() -> String {
  nanoid!(6)
}

pub fn gen_option_id() -> String {
  nanoid!(4)
}

pub fn timestamp() -> i64 {
  chrono::Utc::now().timestamp()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DuplicatedDatabase {
  pub view: DatabaseView,
  pub fields: Vec<Field>,
  pub rows: Vec<CreateRowParams>,
}

impl DuplicatedDatabase {
  pub fn to_json(&self) -> Result<String, DatabaseError> {
    let s = serde_json::to_string(self)?;
    Ok(s)
  }

  pub fn from_json(json: &str) -> Result<Self, DatabaseError> {
    let database = serde_json::from_str(json)?;
    Ok(database)
  }
}
