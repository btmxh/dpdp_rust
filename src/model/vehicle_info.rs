use std::{
    fmt::{Debug, Display},
    path::Path,
};

use serde::Deserialize;

use crate::define_map;

use super::{read_csv, MapType};

#[derive(Clone, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct VehicleId(String);

impl Debug for VehicleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl Display for VehicleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VehicleInfo {
    pub car_num: VehicleId,
    pub capacity: i32,
    pub operation_time: i32,
    pub gps_id: String,
}

impl VehicleInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<VehicleInfoMap> {
        Ok(read_csv::<VehicleInfo>(path)?
            .into_iter()
            .map(|v| (v.car_num.clone(), v))
            .collect::<MapType<_, _>>()
            .into())
    }

    pub fn load_instance(inst: i32) -> anyhow::Result<VehicleInfoMap> {
        Self::load(format!("data/benchmark/instance_{}/vehicle_info.csv", inst))
    }
}

#[test]
fn test_read_all_vehicle_infos() {
    for inst in super::ALL_INSTANCES.clone() {
        assert!(
            VehicleInfo::load(format!("data/benchmark/instance_{}/vehicle_info.csv", inst)).is_ok()
        );
    }
}

define_map!(VehicleId, VehicleInfo, VehicleInfoMap);
