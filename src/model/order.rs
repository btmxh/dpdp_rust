use std::path::Path;

use chrono::{Duration, NaiveTime};
use serde::Deserialize;

use super::{read_csv, ALL_INSTANCES};

#[derive(Debug, Deserialize)]
pub struct Order {
    order_id: String,
    q_standard: i32,
    q_small: i32,
    q_box: i32,
    demand: f32,
    creation_time: NaiveTime,
    committed_completion_time: NaiveTime,
    load_time: Duration,
    unload_time: Duration,
    pickup_id: String,
    delivery_id: String,
}

impl Order {
    pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Vec<Order>> {
        read_csv(path)
    }
}
