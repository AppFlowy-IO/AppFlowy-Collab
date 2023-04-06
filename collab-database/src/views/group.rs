use collab::core::any_array::{ArrayMap, ArrayMapUpdate};
use collab::core::any_map::{AnyMap, AnyMapBuilder};

/// [GroupSettingArray] contains list of [GroupSettingMap]
pub type GroupSettingArray = ArrayMap;
pub type GroupSettingArrayUpdate<'a, 'b> = ArrayMapUpdate<'a, 'b>;

/// [GroupSettingMap] contains list of key/value.
/// One of the key/value represents as the [GroupMap]
pub type GroupSettingMap = AnyMap;
pub type GroupSettingBuilder = AnyMapBuilder;

/// [GroupMap] contains the key/value that represents a group data.
pub type GroupMap = AnyMap;
/// [GroupMapBuilder] is the builder for [GroupMap]
pub type GroupMapBuilder = AnyMapBuilder;
