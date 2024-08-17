use std::collections::HashMap;

use collab::preclude::{Any, ArrayRef};

/// [GroupSettingArray] contains list of [GroupSettingMap]
pub type GroupSettingArray = Vec<Any>;
pub type GroupSettingArrayUpdate = ArrayRef;

/// [GroupSettingMap] contains list of key/value.
/// One of the key/value represents as the [GroupMap]
pub type GroupSettingMap = HashMap<String, Any>;
pub type GroupSettingBuilder = HashMap<String, Any>;

/// [GroupMap] contains the key/value that represents a group data.
pub type GroupMap = HashMap<String, Any>;
/// [GroupMapBuilder] is the builder for [GroupMap]
pub type GroupMapBuilder = HashMap<String, Any>;
