use std::fmt::{Debug, Display};

use chrono::{Duration, NaiveTime};
use serde::{Deserialize, Serialize};

use crate::define_map;

use super::{factory_info::FactoryId, order::OrderId};

#[derive(Debug, Clone)]
pub struct OrderItem {
    pub id: OrderItemId,
    pub demand: i32,
    pub creation_time: NaiveTime,
    pub committed_completion_time: NaiveTime,
    pub load_time: Duration,
    pub unload_time: Duration,
    pub pickup_id: FactoryId,
    pub delivery_id: FactoryId,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrderItemType {
    Standard,
    Small,
    Box,
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderItemId {
    pub order_id: OrderId,
    pub item_type: OrderItemType,
    pub index: i32,
}

impl Debug for OrderItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: String = self.into();
        Debug::fmt(&s, f)
    }
}

impl<'de> Deserialize<'de> for OrderItemId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let mut parts = str.split('_');
        let order_id = OrderId(parts.next().unwrap().to_string());
        let item_type = match parts.next().unwrap() {
            "standard" => OrderItemType::Standard,
            "small" => OrderItemType::Small,
            "box" => OrderItemType::Box,
            _ => unreachable!(),
        };
        let index = parts.next().unwrap().parse().unwrap();
        Ok(OrderItemId {
            order_id,
            item_type,
            index,
        })
    }
}

impl From<&OrderItemId> for String {
    fn from(value: &OrderItemId) -> Self {
        format!(
            "{}_{}_{}",
            value.order_id,
            match value.item_type {
                OrderItemType::Standard => "standard",
                OrderItemType::Small => "small",
                OrderItemType::Box => "box",
            },
            value.index
        )
    }
}

impl Serialize for OrderItemId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s: String = self.into();
        serializer.serialize_str(&s)
    }
}

impl Display for OrderItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: String = self.into();
        write!(f, "{}", s)
    }
}

impl OrderItemType {
    pub fn demand(&self) -> i32 {
        match self {
            OrderItemType::Standard => 4,
            OrderItemType::Small => 2,
            OrderItemType::Box => 1,
        }
    }
}

define_map!(OrderItemId, OrderItem, OrderItemMap);
