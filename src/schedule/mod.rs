pub mod naive;
pub mod noop;
// pub mod rl;

use chrono::NaiveDateTime;
use serde::Serialize;

use crate::{
    model::{
        order_item::{OrderItemId, OrderItemMap},
        vehicle_info::VehicleId,
        MapType,
    },
    simulation::simulator::{OrderItemStateMap, Simulator, VehiclePosition, VehicleRoute},
};

pub trait Scheduler {
    fn schedule(&mut self, args: SchedulerArgs) -> MapType<VehicleId, Vec<VehicleRoute>>;
}

#[derive(Serialize)]
pub struct SchedulerArgs {
    #[serde(skip)]
    pub items: OrderItemMap,
    #[serde(skip)]
    pub item_states: OrderItemStateMap,
    pub vehicle_stacks: MapType<VehicleId, Vec<OrderItemId>>,
    pub vehicle_positions: MapType<VehicleId, VehiclePosition>,
    #[serde(skip)]
    pub static_simulator: Simulator,
    pub time: NaiveDateTime,
    pub elapsed_distance: f32,
}

impl SchedulerArgs {}

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
