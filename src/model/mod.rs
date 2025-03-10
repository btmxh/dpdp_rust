use std::{ops::RangeInclusive, path::Path};

use serde::de::DeserializeOwned;

pub mod factory_info;
pub mod order;
pub mod route_info;
pub mod vehicle_info;

static ALL_INSTANCES: RangeInclusive<i32> = 1..=64;

fn read_csv<T>(path: impl AsRef<Path>) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let mut reader = csv::Reader::from_path(path)?;
    let records: csv::Result<Vec<T>> = reader.deserialize().collect();
    Ok(records?)
}
