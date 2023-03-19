use crate::collab::Collab;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

impl Serialize for Collab {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        let txn = self.transact();
        map.serialize_entry("attributes", &self.to_json(&txn))?;
        map.end()
    }
}
