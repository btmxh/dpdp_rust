pub mod naive;

use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::{
    model::{
        order_item::{OrderItemId, OrderItemMap},
        vehicle_info::VehicleId,
        MapType,
    },
    simulation::simulator::VehicleRoute,
};

pub trait Scheduler {
    fn schedule(
        &mut self,
        unallocated_order_items: OrderItemMap,
        ongoing_order_items: OrderItemMap,
        vehicle_stacks: MapType<VehicleId, Vec<OrderItemId>>,
        time: NaiveDateTime,
    ) -> MapType<VehicleId, Vec<VehicleRoute>>;
}

pub fn deduplicate(plans: &mut MapType<VehicleId, Vec<VehicleRoute>>) {
    for plan in plans.values_mut() {
        let old_plan = std::mem::take(plan);
        for route in old_plan {
            if let Some(tail) = plan.last_mut() {
                if let Err(route) = tail.try_merge(route) {
                    plan.push(route);
                }
            } else {
                plan.push(route);
            }
        }
    }
}
