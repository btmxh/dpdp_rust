use std::{
    fmt::{Debug, Display},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::define_map;

use super::{read_csv, MapType};

#[derive(Clone, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FactoryId(pub String);

impl Debug for FactoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for FactoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactoryInfo {
    pub factory_id: FactoryId,
    pub longitude: f64,
    pub latitude: f64,
    pub port_num: i32,
}

impl FactoryInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<FactoryInfoMap> {
        Ok(read_csv::<FactoryInfo>(path)?
            .into_iter()
            .map(|info| (info.factory_id.clone(), info))
            .collect::<MapType<_, _>>()
            .into())
    }

    pub fn load_std() -> anyhow::Result<FactoryInfoMap> {
        Self::load("data/benchmark/factory_info.csv")
    }
}

define_map!(FactoryId, FactoryInfo, FactoryInfoMap);

#[test]
fn test_load_factory_info() {
    assert!(FactoryInfo::load_std().is_ok());
}
