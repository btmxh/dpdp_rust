use std::path::Path;

use serde::Deserialize;

use super::read_csv;

#[derive(Debug, Deserialize)]
pub struct FactoryInfo {
    factory_id: String,
    longitude: f64,
    latitude: f64,
    port_num: i32,
}

impl FactoryInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Vec<FactoryInfo>> {
        read_csv(path)
    }
}

#[test]
fn test_load_factory_info() {
    assert!(FactoryInfo::load("data/benchmark/factory_info.csv").is_ok());
}
