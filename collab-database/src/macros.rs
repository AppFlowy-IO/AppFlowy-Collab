#[macro_export]
macro_rules! impl_str_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1<T: AsRef<str>>(self, value: T) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value.as_ref());
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
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }

    pub fn $setter2(self, value: Option<i64>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_i32_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: i32) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }

    pub fn $setter2(self, value: Option<i32>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_u8_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: u8) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }

    pub fn $setter2(self, value: Option<u8>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_bool_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: bool) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }
    pub fn $setter2(self, value: Option<bool>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_any_update {
  ($setter1: ident,  $setter2: ident,  $key:expr, $value: ident) => {
    pub fn $setter1(self, value: $value) -> Self {
      self.map_ref.insert_with_txn(self.txn, $key, value);
      self
    }
    pub fn $setter2(self, value: Option<$value>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert_with_txn(self.txn, $key, value);
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_order_update {
  ($setter1: ident,  $setter2: ident,  $setter3: ident, $key:expr, $ty:ident, $array_ty:ident) => {
    pub fn $setter1(self, orders: Vec<$ty>) -> Self {
      let array_ref = self
        .map_ref
        .get_or_insert_array_with_txn::<$ty>(self.txn, $key);
      let array = $array_ty::new(array_ref);
      array.extends_with_txn(self.txn, orders);
      self
    }

    pub fn $setter2<T: Into<$ty>>(self, order: T) -> Self {
      let order = order.into();
      if let Some(array) = self
        .map_ref
        .get_array_ref_with_txn(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        array.push_back(self.txn, order);
      }
      self
    }

    pub fn $setter3(self, row_id: &str) -> Self {
      if let Some(array) = self
        .map_ref
        .get_array_ref_with_txn(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        array.remove_with_txn(self.txn, row_id);
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
