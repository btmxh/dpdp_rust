use std::path::Path;

use serde::Deserialize;

use super::read_csv;

#[derive(Debug, Deserialize)]
pub struct VehicleInfo {
    car_num: String,
    capacity: i32,
    operation_time: i32,
    gps_id: String,
}

impl VehicleInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Vec<VehicleInfo>> {
        read_csv(path)
    }
}
