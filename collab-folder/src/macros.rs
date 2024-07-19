#[macro_export]
macro_rules! impl_str_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1<T: AsRef<str>>(self, value: T) -> Self {
      self.map_ref.insert(self.txn, $key, value.as_ref());
      self
    }
    pub fn $setter2<T: AsRef<str>>(self, value: Option<T>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, value.as_ref());
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_option_str_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: Option<String>) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }
    pub fn $setter2<T: AsRef<str>>(self, value: Option<T>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value.as_ref());
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_i64_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: i64) -> Self {
      self.map_ref.insert(self.txn, $key, value);
      self
    }

    pub fn $setter2(self, value: Option<i64>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_option_i64_update {
  ($setter1: ident, $key: expr) => {
    pub fn $setter1(self, value: Option<i64>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_bool_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: bool) -> Self {
      self.map_ref.insert(self.txn, $key, value);
      self
    }
    pub fn $setter2(self, value: Option<bool>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_any_update {
  ($setter1: ident,  $setter2: ident,  $key:expr, $value: ident) => {
    pub fn $setter1(self, value: $value) -> Self {
      self.map_ref.insert(self.txn, $key, value);
      self
    }
    pub fn $setter2(self, value: Option<$value>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_array_update {
  ($setter: ident, $key: expr, $value: ident) => {
    pub fn $setter(self, value: $value) -> Self {
      self
        .map_ref
        .insert_array_with_txn(self.txn, $key, value.into());
      self
    }
  };
}

#[macro_export]
macro_rules! impl_section_op {
  ($section_type:expr, $set_fn:ident, $add_fn:ident, $delete_fn:ident, $get_my_fn:ident, $get_all_fn:ident, $remove_all_fn:ident) => {
    // Add view IDs as either favorites or recents
    pub fn $add_fn(&self, txn: &mut TransactionMut, ids: Vec<String>) {
      for id in ids {
        self
          .views
          .update_view(txn, &id, |update| update.$set_fn(true).done());
      }
    }

    pub fn $delete_fn(&self, txn: &mut TransactionMut, ids: Vec<String>) {
      for id in ids {
        self
          .views
          .update_view(txn, &id, |update| update.$set_fn(false).done());
      }
    }

    // Get all section items for the current user
    pub fn $get_my_fn<T: ReadTxn>(&self, txn: &T) -> Vec<SectionItem> {
      self
        .section
        .section_op(txn, $section_type)
        .map(|op| op.get_all_section_item(txn))
        .unwrap_or_default()
    }

    // Get all sections
    pub fn $get_all_fn<T: ReadTxn>(&self, txn: &T) -> Vec<SectionItem> {
      self
        .section
        .section_op(txn, $section_type)
        .map(|op| op.get_sections(txn))
        .unwrap_or_default()
        .into_iter()
        .flat_map(|(_user_id, items)| items)
        .collect::<Vec<_>>()
    }

    // Clear all items in a section
    pub fn $remove_all_fn(&self, txn: &mut TransactionMut) {
      if let Some(op) = self.section.section_op(txn, $section_type) {
        op.clear(txn)
      }
    }
  };
}
