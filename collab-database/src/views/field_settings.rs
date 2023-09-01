use std::{
  collections::HashMap,
  ops::{Deref, DerefMut},
};

use collab::core::any_map::{AnyMap, AnyMapBuilder};
use collab::preclude::{Map, MapRef, MapRefExtension, ReadTxn, TransactionMut, YrsValue};
use serde::{Deserialize, Serialize};

pub type FieldSettingsMap = AnyMap;
pub type FieldSettingsMapBuilder = AnyMapBuilder;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FieldSettingsByFieldIdMap(HashMap<String, FieldSettingsMap>);

impl FieldSettingsByFieldIdMap {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_inner(self) -> HashMap<String, FieldSettingsMap> {
    self.0
  }

  pub fn fill_map_ref(self, txn: &mut TransactionMut, map_ref: &MapRef) {
    self
      .into_inner()
      .into_iter()
      .for_each(|(field_id, settings)| {
        let field_settings_map_ref = map_ref.get_or_create_map_with_txn(txn, &field_id);
        settings.fill_map_ref(txn, &field_settings_map_ref);
      });
  }

  /// Returns a [FieldSettingsMap] from FieldSettingsByIdMap based on the field ID
  pub fn get_settings_with_field_id(&self, field_id: &str) -> Option<&FieldSettingsMap> {
    self.get(field_id)
  }
}

impl<T: ReadTxn> From<(&'_ T, &MapRef)> for FieldSettingsByFieldIdMap {
  fn from(params: (&'_ T, &MapRef)) -> Self {
    let mut this = Self::new();
    params.1.iter(params.0).for_each(|(k, v)| {
      if let YrsValue::YMap(map_ref) = v {
        this.insert(k.to_string(), (params.0, &map_ref).into());
      }
    });
    this
  }
}

impl From<HashMap<String, FieldSettingsMap>> for FieldSettingsByFieldIdMap {
  fn from(data: HashMap<String, FieldSettingsMap>) -> Self {
    Self(data)
  }
}

impl Deref for FieldSettingsByFieldIdMap {
  type Target = HashMap<String, FieldSettingsMap>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for FieldSettingsByFieldIdMap {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}
