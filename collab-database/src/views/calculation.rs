use serde_json::{Map, Value};

pub type CalculationArray = Vec<Value>;
pub type CalculationMap = Map<String, Value>;
pub type CalculationMapBuilder = Map<String, Value>;
