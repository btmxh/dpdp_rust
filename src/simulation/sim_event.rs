use chrono::{Duration, NaiveDateTime};
use serde::Serialize;

use crate::model::{
    factory_info::FactoryId,
    order::OrderId,
    order_item::{OrderItemId, OrderItemMap},
    vehicle_info::VehicleId,
    Map as _,
};

use super::event_queue::Event;

#[derive(Debug, Clone, Serialize)]
pub struct VehicleWork {
    pub load_items: Vec<OrderItemId>,
    pub unload_items: Vec<OrderItemId>,
    pub load_time: Duration,
    pub unload_time: Duration,
}

impl VehicleWork {
    pub fn new(
        order_items: &OrderItemMap,
        pickup_items: Vec<OrderItemId>,
        delivery_items: Vec<OrderItemId>,
    ) -> Self {
        let load_time_per_box = Duration::minutes(1);
        let unload_time_per_box = load_time_per_box;

        Self {
            load_time: load_time_per_box
                * pickup_items
                    .iter()
                    .map(|i| order_items.gets(i).demand)
                    .sum(),
            unload_time: unload_time_per_box
                * delivery_items
                    .iter()
                    .map(|i| order_items.gets(i).demand)
                    .sum(),
            load_items: pickup_items,
            unload_items: delivery_items,
        }
    }

    pub fn new_load(order_items: &OrderItemMap, pickup_items: Vec<OrderItemId>) -> Self {
        Self::new(order_items, pickup_items, vec![])
    }

    pub fn new_unload(order_items: &OrderItemMap, pickup_items: Vec<OrderItemId>) -> Self {
        Self::new(order_items, vec![], pickup_items)
    }

    pub fn delta_demand(&self, order_items: &OrderItemMap) -> i32 {
        self.load_items
            .iter()
            .map(|i| order_items.gets(i).demand)
            .sum::<i32>()
            - self
                .unload_items
                .iter()
                .map(|i| order_items.gets(i).demand)
                .sum::<i32>()
    }

    pub fn merge(&mut self, work: VehicleWork) {
        self.load_items.extend(work.load_items);
        self.unload_items.extend(work.unload_items);
        self.load_time += work.load_time;
        self.unload_time += work.unload_time;
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SimulatorEventData {
    OrderArrival {
        order_id: OrderId,
        order_item_ids: Vec<OrderItemId>,
    },
    VehicleArrival {
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        work: VehicleWork,
    },
    VehicleApproachedDock {
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        work: VehicleWork,
    },
    FinishLoading {
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        delivered_items: Vec<OrderItemId>,
    },
    UpdateTimestep,
}

impl Event for (SimulatorEventData, NaiveDateTime) {
    fn time(&self) -> chrono::NaiveDateTime {
        self.1
    }
}
