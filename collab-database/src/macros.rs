#[macro_export]
macro_rules! impl_str_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1<T: AsRef<str>>(self, value: T) -> Self {
      self.map_ref.try_update(self.txn, $key, value.as_ref());
      self
    }
    pub fn $setter2<T: AsRef<str>>(self, value: Option<T>) -> Self {
      if let Some(value) = value {
        self.map_ref.try_update(self.txn, $key, value.as_ref());
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_replace_str_update {
  ($replace_str: ident, $key: expr) => {
    pub fn $replace_str<F: FnOnce(&str) -> String>(self, f: F) -> Self {
      if let Some(s) = self.map_ref.get_str_with_txn(self.txn, $key) {
        let new_id = f(&s);
        self.map_ref.insert_with_txn(self.txn, $key, new_id);
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
      self.map_ref.insert(self.txn, $key, Any::BigInt(value));
      self
    }

    pub fn $setter2(self, value: Option<i64>) -> Self {
      if let Some(value) = value {
        self.map_ref.insert(self.txn, $key, Any::BigInt(value));
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_i32_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: i32) -> Self {
      self
        .map_ref
        .insert(self.txn, $key, Any::BigInt(value as i64));
      self
    }

    pub fn $setter2(self, value: Option<i32>) -> Self {
      if let Some(value) = value {
        self
          .map_ref
          .insert(self.txn, $key, Any::BigInt(value as i64));
      }
      self
    }
  };
}

#[macro_export]
macro_rules! impl_u8_update {
  ($setter1: ident, $setter2: ident, $key: expr) => {
    pub fn $setter1(self, value: u8) -> Self {
      self
        .map_ref
        .insert(self.txn, $key, Any::BigInt(value as i64));
      self
    }

    pub fn $setter2(self, value: Option<u8>) -> Self {
      if let Some(value) = value {
        self
          .map_ref
          .insert(self.txn, $key, Any::BigInt(value as i64));
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
  ($setter1: ident, $setter2: ident, $key:expr, $value: ident) => {
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
macro_rules! impl_order_update {
  ($set_orders: ident,
    $remove: ident,
    $move_to: ident,
    $insert: ident,
    $iter_mut: ident,
    $key: expr,
    $ty: ident,
    $array_ty: ident
  ) => {
    pub fn $set_orders(self, orders: Vec<$ty>) -> Self {
      let array_ref: ArrayRef = self.map_ref.get_or_init(self.txn, $key);
      let array = $array_ty::new(array_ref);
      array.extends_with_txn(self.txn, orders);
      self
    }

    pub fn $remove(self, id: &str) -> Self {
      if let Some(array) = self
        .map_ref
        .get_with_txn::<_, ArrayRef>(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        array.remove_with_txn(self.txn, id);
      }
      self
    }

    pub fn $move_to(self, from_id: &str, to_id: &str) -> Self {
      if let Some(array) = self
        .map_ref
        .get_with_txn::<_, ArrayRef>(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        array.move_to(self.txn, from_id, to_id);
      }
      self
    }

    pub fn $insert<T: Into<$ty>>(self, object: T, position: &OrderObjectPosition) -> Self {
      let object = object.into();
      if let Some(array) = self
        .map_ref
        .get_with_txn::<_, ArrayRef>(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        match position {
          OrderObjectPosition::Start => array.push_front_with_txn(self.txn, object),
          OrderObjectPosition::Before(next_object_id) => {
            array.insert_before_with_txn(self.txn, object, &next_object_id)
          },
          OrderObjectPosition::After(prev_object_id) => {
            array.insert_after_with_txn(self.txn, object, &prev_object_id)
          },
          OrderObjectPosition::End => array.push_back_with_txn(self.txn, object),
        };
      }
      self
    }

    pub fn $iter_mut<F: FnMut(&mut $ty)>(self, mut f: F) -> Self {
      if let Some(array) = self
        .map_ref
        .get_with_txn::<_, ArrayRef>(self.txn, $key)
        .map(|array_ref| $array_ty::new(array_ref))
      {
        for mut row_order in array.get_objects_with_txn(self.txn) {
          array.remove_with_txn(self.txn, row_order.id.as_str());
          f(&mut row_order);
          array.push_back(self.txn, row_order);
        }
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
        .insert(self.txn, $key, ArrayPrelim::new(value.into()));
      self
    }
  };
}
