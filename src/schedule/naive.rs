use chrono::NaiveDateTime;
use rand::seq::IndexedRandom;

use crate::{
    model::{
        order::{Order, OrderId},
        order_item::{OrderItem, OrderItemId, OrderItemMap},
        vehicle_info::{VehicleId, VehicleInfo, VehicleInfoMap},
        Map, MapType,
    },
    simulation::{sim_event::LoadUnloadWork, simulator::VehicleRoute},
};

use super::{deduplicate, Scheduler};

pub struct NaiveScheduler {
    vehicles: VehicleInfoMap,
    order_items: OrderItemMap,
}

impl NaiveScheduler {
    pub fn new(inst_num: i32) -> anyhow::Result<Self> {
        Ok(Self {
            vehicles: VehicleInfo::load_instance(inst_num)?,
            order_items: Order::load_instance(inst_num)?
                .values()
                .flat_map(Order::into_items)
                .map(|o| (o.id.clone(), o))
                .collect::<MapType<_, _>>()
                .into(),
        })
    }
}

impl Scheduler for NaiveScheduler {
    fn schedule(
        &mut self,
        unallocated_order_items: OrderItemMap,
        ongoing_order_items: OrderItemMap,
        vehicle_stacks: MapType<VehicleId, Vec<OrderItemId>>,
        time: NaiveDateTime,
    ) -> MapType<VehicleId, Vec<VehicleRoute>> {
        let mut schedule = MapType::new();
        for (vid, items) in vehicle_stacks {
            let plan: &mut Vec<VehicleRoute> = schedule.entry(vid).or_default();
            for item_id in items.into_iter() {
                let item = self.order_items.gets(&item_id);
                plan.push(VehicleRoute::new(
                    item.delivery_id.clone(),
                    LoadUnloadWork::new_unload(&self.order_items, vec![item_id]),
                ));
            }
        }

        let mut orders: MapType<OrderId, Vec<OrderItem>> = MapType::new();
        for (_, item) in unallocated_order_items {
            orders
                .entry(item.id.order_id.clone())
                .or_default()
                .push(item);
        }

        let vehicles: Vec<_> = self.vehicles.iter().collect();
        let mut current_vehicle_itr = vehicles.into_iter().cycle();
        let (mut vid, mut vehicle_info) = current_vehicle_itr.next().unwrap();

        for (_, items) in orders {
            let mut pending_items = vec![];
            let mut total_demand = 0i32;

            let pickup_id = items.first().unwrap().pickup_id.clone();
            let delivery_id = items.first().unwrap().delivery_id.clone();

            for item in items {
                if total_demand + item.demand > vehicle_info.capacity {
                    let plan = schedule.entry(vid.clone()).or_default();
                    plan.push(VehicleRoute::new(
                        pickup_id.clone(),
                        LoadUnloadWork::new_load(&self.order_items, pending_items.clone()),
                    ));
                    plan.push(VehicleRoute::new(
                        delivery_id.clone(),
                        LoadUnloadWork::new_unload(&self.order_items, pending_items.clone()),
                    ));

                    pending_items.clear();
                    total_demand = 0;
                    (vid, vehicle_info) = current_vehicle_itr.next().unwrap();
                }

                assert!(item.demand < vehicle_info.capacity);
                pending_items.push(item.id);
                total_demand += item.demand;
            }

            if !pending_items.is_empty() {
                let plan = schedule.entry(vid.clone()).or_default();
                plan.push(VehicleRoute::new(
                    pickup_id.clone(),
                    LoadUnloadWork::new_load(&self.order_items, pending_items.clone()),
                ));
                plan.push(VehicleRoute::new(
                    delivery_id.clone(),
                    LoadUnloadWork::new_unload(&self.order_items, pending_items.clone()),
                ));

                pending_items.clear();
                (vid, vehicle_info) = current_vehicle_itr.next().unwrap();
            }
        }

        deduplicate(&mut schedule);

        schedule
    }
}
