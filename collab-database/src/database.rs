use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::id_gen::ID_GEN;
use crate::meta::MetaMap;
use crate::rows::{Row, RowId, RowMap};
use crate::views::{
  CreateDatabaseParams, CreateViewParams, DatabaseView, GroupSettingMap, RowOrder, ViewMap,
};
use collab::preclude::{
  Collab, JsonValue, MapRefExtension, MapRefWrapper, ReadTxn, TransactionMut,
};
use nanoid::nanoid;
use std::rc::Rc;

pub struct Database {
  #[allow(dead_code)]
  inner: Collab,
  pub(crate) root: MapRefWrapper,
  pub rows: Rc<RowMap>,
  pub views: Rc<ViewMap>,
  pub fields: Rc<FieldMap>,
  pub metas: Rc<MetaMap>,
}

const DATABASE_ID: &str = "id";
const DATABASE: &str = "database";
const FIELDS: &str = "fields";
const ROWS: &str = "rows";
const VIEWS: &str = "views";
const METAS: &str = "metas";
const DATABASE_INLINE_VIEW: &str = "iid";

pub struct DatabaseContext {
  pub collab: Collab,
}

impl Database {
  pub fn create_with_view(
    database_id: &str,
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Self, DatabaseError> {
    let this = Self::get_or_create(database_id, context)?;
    let (rows, fields, params) = params.split();
    this.root.with_transact_mut(|txn| {
      this.set_inline_view_with_txn(txn, &params.view_id);
      for row in rows {
        this.rows.insert_row_with_txn(txn, row);
      }
      for field in fields {
        this.fields.insert_field_with_txn(txn, field);
      }
    });
    this.create_view(params);
    Ok(this)
  }

  pub fn get_or_create(database_id: &str, context: DatabaseContext) -> Result<Self, DatabaseError> {
    if database_id.is_empty() {
      return Err(DatabaseError::InvalidDatabaseID);
    }
    let collab = context.collab;
    let (database, fields, rows, views, metas) = collab.with_transact_mut(|txn| {
      // { DATABASE: {:} }
      let database = collab
        .get_map_with_txn(txn, vec![DATABASE])
        .unwrap_or_else(|| collab.create_map_with_txn(txn, DATABASE));

      database.insert_str_with_txn(txn, DATABASE_ID, database_id);

      // { DATABASE: { FIELDS: {:} } }
      let fields = collab
        .get_map_with_txn(txn, vec![DATABASE, FIELDS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, FIELDS));

      // { DATABASE: { FIELDS: {:}, ROWS: {:} } }
      let rows = collab
        .get_map_with_txn(txn, vec![DATABASE, ROWS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, ROWS));
      let rows = RowMap::new_with_txn(txn, rows);

      // { DATABASE: { FIELDS: {:}, ROWS: {:}, VIEWS: {:} } }
      let views = collab
        .get_map_with_txn(txn, vec![DATABASE, VIEWS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, VIEWS));

      // { DATABASE: { FIELDS: {:}, ROWS: {:}, VIEWS: {:}, METAS: {:} } }
      let metas = collab
        .get_map_with_txn(txn, vec![DATABASE, METAS])
        .unwrap_or_else(|| database.insert_map_with_txn(txn, METAS));

      (database, fields, rows, views, metas)
    });
    let views = ViewMap::new(views);
    let fields = FieldMap::new(fields);
    let metas = MetaMap::new(metas);

    Ok(Self {
      inner: collab,
      root: database,
      rows: Rc::new(rows),
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

  pub fn push_row(&self, row: Row) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.add_row_order(&row);
      });
      self.rows.insert_row_with_txn(txn, row);
    })
  }

  pub fn insert_row(&self, row: Row, prev_row_id: Option<RowId>) {
    self.root.with_transact_mut(|txn| {
      self.insert_row_with_txn(txn, row, prev_row_id);
    });
  }

  pub fn insert_row_with_txn(
    &self,
    txn: &mut TransactionMut,
    row: Row,
    prev_row_id: Option<RowId>,
  ) {
    self.views.update_all_views_with_txn(txn, |update| {
      let prev_row_id = prev_row_id.map(|value| value.to_string());
      update.insert_row_order(&row, prev_row_id);
    });
    self.rows.insert_row_with_txn(txn, row);
  }

  pub fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let txn = self.root.transact();
    let row_orders = self
      .views
      .get_view_with_txn(&txn, view_id)
      .map(|view| view.row_orders)
      .unwrap_or_default();

    self.get_rows_in_order_with_txn(&txn, &row_orders)
  }

  pub fn get_rows_in_order_with_txn<T: ReadTxn>(
    &self,
    txn: &T,
    row_orders: &[RowOrder],
  ) -> Vec<Row> {
    row_orders
      .iter()
      .flat_map(|row_order| self.rows.get_row_with_txn(txn, row_order.id))
      .collect::<Vec<Row>>()
  }

  pub fn remove_row(&self, row_id: &RowId) {
    let row_id = row_id.to_string();
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_row_order(&row_id);
      });
      self.rows.delete_row_with_txn(txn, &row_id);
    })
  }

  pub fn insert_field(&self, field: Field) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.add_field_order(&field);
      });
      self.fields.insert_field_with_txn(txn, field);
    })
  }

  pub fn delete_field(&self, field_id: &str) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_field_order(field_id);
      });
      self.fields.delete_field_with_txn(txn, field_id);
    })
  }

  pub fn add_group_setting(&self, view_id: &str, group_setting: impl Into<GroupSettingMap>) {
    self.views.update_view(view_id, |update| {
      update.update_groups(|group_update| {
        group_update.push(group_setting.into());
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

  pub fn create_view(&self, params: CreateViewParams) {
    self.root.with_transact_mut(|txn| {
      let field_orders = self.fields.get_all_field_orders_with_txn(txn);
      let row_orders = self.rows.get_all_row_orders_with_txn(txn);
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
    })
  }

  pub fn get_view(&self, view_id: &str) -> Option<DatabaseView> {
    let txn = self.root.transact();
    self.views.get_view_with_txn(&txn, view_id)
  }

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

  pub fn duplicate_row(&self, row_id: RowId) {
    self.root.with_transact_mut(|txn| {
      if let Some(mut row) = self.rows.get_row_with_txn(txn, row_id) {
        row.id = gen_row_id();
        self.insert_row_with_txn(txn, row, Some(row_id));
      }
    });
    todo!()
  }

  pub fn duplicate_data(&self) -> DuplicatedDatabase {
    let inline_view_id = self.get_inline_view_id();
    let mut view = self.views.get_view(&inline_view_id).unwrap();
    view.id = gen_database_view_id();
    let rows = self.rows.get_all_rows();
    let fields = self.fields.get_all_fields();
    DuplicatedDatabase { view, rows, fields }
  }

  pub fn to_json_value(&self) -> JsonValue {
    let database_serde = DatabaseSerde::from_database(self);
    serde_json::to_value(&database_serde).unwrap()
  }

  pub fn is_inline_view(&self, view_id: &str) -> bool {
    let inline_view_id = self.get_inline_view_id();
    inline_view_id == view_id
  }

  fn set_inline_view_with_txn(&self, txn: &mut TransactionMut, view_id: &str) {
    self
      .metas
      .insert_str_with_txn(txn, DATABASE_INLINE_VIEW, view_id);
  }

  /// The inline view is the view that create with the database when initializing
  fn get_inline_view_id(&self) -> String {
    let txn = self.root.transact();
    // It's safe to unwrap because each database inline view id was set
    // when initializing the database
    self
      .metas
      .get_str_with_txn(&txn, DATABASE_INLINE_VIEW)
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

pub struct DuplicatedDatabase {
  pub view: DatabaseView,
  pub rows: Vec<Row>,
  pub fields: Vec<Field>,
}
