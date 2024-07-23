use collab::preclude::ArrayRef;
use serde_json::{Map, Value};

/// [GroupSettingArray] contains list of [GroupSettingMap]
pub type GroupSettingArray = Vec<Value>;
pub type GroupSettingArrayUpdate = ArrayRef;

/// [GroupSettingMap] contains list of key/value.
/// One of the key/value represents as the [GroupMap]
pub type GroupSettingMap = Map<String, Value>;
pub type GroupSettingBuilder = Map<String, Value>;

/// [GroupMap] contains the key/value that represents a group data.
pub type GroupMap = Map<String, Value>;
/// [GroupMapBuilder] is the builder for [GroupMap]
pub type GroupMapBuilder = Map<String, Value>;
