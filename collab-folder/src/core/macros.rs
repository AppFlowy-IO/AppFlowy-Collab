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
macro_rules! impl_array_update {
    ($setter: ident, $key: expr, $value: ident) => {
        pub fn $setter(self, value: $value) -> Self {
            self.map_ref
                .insert_array_with_txn(self.txn, $key, value.into());
            self
        }
    };
}
