use crate::block::{Blocks, CreateRowParams};
use crate::database_serde::DatabaseSerde;
use crate::error::DatabaseError;
use crate::fields::{Field, FieldMap};
use crate::id_gen::ID_GEN;
use crate::meta::MetaMap;
use crate::rows::{BlockId, Row, RowId, RowUpdate};
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
  pub fn create_with_view(
    database_id: &str,
    params: CreateDatabaseParams,
    context: DatabaseContext,
  ) -> Result<Self, DatabaseError> {
    let this = Self::get_or_create(database_id, context)?;
    let (rows, fields, params) = params.split();
    let row_orders = this.blocks.create_rows(rows);

    this.root.with_transact_mut(|txn| {
      this.set_inline_view_with_txn(txn, &params.view_id);
      for field in fields {
        this.fields.insert_field_with_txn(txn, field);
      }
      this.create_inline_view_with_txn(txn, params, row_orders);
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
        update.add_row_order(&row_order);
      });
    });
    Some(row_order)
  }

  pub fn create_row(&self, params: CreateRowParams) {
    self.root.with_transact_mut(|txn| {
      self.create_row_with_txn(txn, params);
    });
  }

  pub fn create_row_with_txn(&self, txn: &mut TransactionMut, params: CreateRowParams) {
    let prev_row_id = params.prev_row_id.map(|value| value.to_string());
    if let Some(row_order) = self.blocks.create_row(params) {
      self.views.update_all_views_with_txn(txn, |update| {
        update.insert_row_order(&row_order, prev_row_id.clone());
      });
    }
  }

  pub fn get_row<R, B>(&self, row_id: R, block_id: B) -> Option<Row>
  where
    R: Into<RowId>,
    B: Into<BlockId>,
  {
    let block = self.blocks.get_block(block_id)?;
    block.get_row(row_id)
  }

  pub fn get_rows_for_view(&self, view_id: &str) -> Vec<Row> {
    let txn = self.root.transact();
    self.get_rows_for_view_with_txn(&txn, view_id)
  }

  pub fn get_rows_for_view_with_txn<T: ReadTxn>(&self, txn: &T, view_id: &str) -> Vec<Row> {
    let row_orders = self.views.get_view_row_orders(txn, view_id);
    self.blocks.get_rows_from_row_orders(&row_orders)
  }

  pub fn remove_row(&self, row_id: RowId, block_id: BlockId) {
    self.root.with_transact_mut(|txn| {
      self.views.update_all_views_with_txn(txn, |update| {
        update.remove_row_order(&row_id.to_string());
      });
      self.blocks.remove_row(row_id, block_id);
    })
  }

  pub fn update_row<R, B, F>(&self, row_id: R, block_id: B, f: F)
  where
    F: FnOnce(RowUpdate),
    R: Into<RowId>,
    B: Into<BlockId>,
  {
    if let Some(block) = self.blocks.get_block(block_id) {
      block.update_row(row_id, f);
    }
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
      let inline_view_id = self.get_inline_view_id_with_txn(txn);
      let row_orders = self.views.get_view_row_orders(txn, &inline_view_id);
      self.create_inline_view_with_txn(txn, params, row_orders);
    })
  }

  pub fn create_inline_view_with_txn(
    &self,
    txn: &mut TransactionMut,
    params: CreateViewParams,
    row_orders: Vec<RowOrder>,
  ) {
    let field_orders = self.fields.get_all_field_orders_with_txn(txn);
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

  pub fn duplicate_row(&self, row_id: RowId, block_id: BlockId) {
    self.root.with_transact_mut(|txn| {
      if let Some(mut row) = self.blocks.get_row_with_txn(txn, row_id, block_id) {
        row.id = gen_row_id();
        let mut params: CreateRowParams = row.into();
        params.prev_row_id = Some(row_id);
        self.create_row_with_txn(txn, params);
      }
    });
    todo!()
  }

  pub fn duplicate_data(&self) -> DuplicatedDatabase {
    let inline_view_id = self.get_inline_view_id();
    let mut view = self.views.get_view(&inline_view_id).unwrap();
    view.id = gen_database_view_id();
    let fields = self.fields.get_all_fields();
    DuplicatedDatabase { view, fields }
  }

  pub fn to_json_value(&self) -> JsonValue {
    let database_serde = DatabaseSerde::from_database(self);
    serde_json::to_value(&database_serde).unwrap()
  }

  pub fn is_inline_view(&self, view_id: &str) -> bool {
    let inline_view_id = self.get_inline_view_id();
    inline_view_id == view_id
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

pub struct DuplicatedDatabase {
  pub view: DatabaseView,
  pub fields: Vec<Field>,
}
