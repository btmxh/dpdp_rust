use std::{
    fmt::{Debug, Display},
    path::Path,
};

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Deserialize;

use crate::define_map;

use super::{
    factory_info::FactoryId,
    order_item::{OrderItem, OrderItemId, OrderItemType},
    read_csv, MapType,
};

#[derive(Clone, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderId(pub(super) String);

impl Debug for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

impl Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    pub order_id: OrderId,
    pub q_standard: i32,
    pub q_small: i32,
    pub q_box: i32,
    pub demand: f32,
    #[serde(deserialize_with = "super::parse_naive_time")]
    pub creation_time: NaiveTime,
    #[serde(deserialize_with = "super::parse_naive_time")]
    committed_completion_time: NaiveTime,
    #[serde(deserialize_with = "super::parse_duration")]
    pub load_time: Duration,
    #[serde(deserialize_with = "super::parse_duration")]
    pub unload_time: Duration,
    pub pickup_id: FactoryId,
    pub delivery_id: FactoryId,
}

impl Order {
    pub fn committed_completion_time(&self, date: NaiveDate) -> NaiveDateTime {
        let mut date_time = date.and_time(self.committed_completion_time);
        if self.creation_time > self.committed_completion_time {
            date_time += Duration::days(1);
        }
        date_time
    }

    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<OrderMap> {
        Ok(read_csv::<Order>(path)?
            .into_iter()
            .map(|o| (o.order_id.clone(), o))
            .collect::<MapType<_, _>>()
            .into())
    }

    pub fn load_instance(inst: i32) -> anyhow::Result<OrderMap> {
        Self::load(format!("data/benchmark/instance_{}/orders.csv", inst))
    }

    fn create_item(&self, typ: OrderItemType, index: i32) -> OrderItem {
        OrderItem {
            id: OrderItemId {
                order_id: self.order_id.clone(),
                item_type: typ,
                index,
            },
            demand: typ.demand(),
            creation_time: self.creation_time,
            committed_completion_time: self.committed_completion_time,
            load_time: self.load_time,
            unload_time: self.unload_time,
            pickup_id: self.pickup_id.clone(),
            delivery_id: self.delivery_id.clone(),
        }
    }

    pub fn into_items(&self) -> Vec<OrderItem> {
        let mut items = Vec::new();
        for i in 0..self.q_standard {
            items.push(self.create_item(OrderItemType::Standard, i));
        }
        for i in 0..self.q_small {
            items.push(self.create_item(OrderItemType::Small, i));
        }
        for i in 0..self.q_box {
            items.push(self.create_item(OrderItemType::Box, i));
        }
        items
    }

    pub fn calc_demand(&self) -> i32 {
        self.q_standard * OrderItemType::Standard.demand()
            + self.q_small * OrderItemType::Small.demand()
            + self.q_box * OrderItemType::Box.demand()
    }
}

define_map!(OrderId, Order, OrderMap);

#[test]
fn test_read_all_orders() {
    for inst in super::ALL_INSTANCES.clone() {
        assert!(Order::load_instance(inst).is_ok());
    }
}
