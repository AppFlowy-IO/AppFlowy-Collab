use serde_json::{Map, Value};

pub type FilterArray = Vec<Value>;
pub type FilterMap = Map<String, Value>;
pub type FilterMapBuilder = Map<String, Value>;
