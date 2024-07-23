use serde_json::{Map, Value};

pub type SortArray = Vec<Value>;
pub type SortMap = Map<String, Value>;
pub type SortMapBuilder = Map<String, Value>;
