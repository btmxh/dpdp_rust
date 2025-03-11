use std::sync::Arc;

use serde::Deserialize;

pub type FastStr = Arc<str>;

trait FastStrMarker {}

impl FastStrMarker for Arc<str> {}

impl<T: FastStrMarker> Deserialize for T {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)
    }
}
