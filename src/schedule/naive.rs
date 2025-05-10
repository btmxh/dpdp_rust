use crate::{
    model::{
        order::{Order, OrderId},
        order_item::{OrderItem, OrderItemMap},
        vehicle_info::{VehicleId, VehicleInfo, VehicleInfoMap},
        Map, MapType,
    },
    simulation::{
        sim_event::VehicleWork,
        simulator::{OrderItemState, VehicleRoute},
    },
};

use super::{deduplicate, Scheduler, SchedulerArgs};

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

    pub fn schedule_opt(
        &mut self,
        SchedulerArgs {
            items,
            item_states,
            vehicle_stacks,
            ..
        }: SchedulerArgs,
        allocate: bool,
    ) -> MapType<VehicleId, Vec<VehicleRoute>> {
        // println all items.id
        let ids: Vec<_> = items.iter().map(|i| i.0).collect();
        println!("items: {ids:?}");
        let mut schedule = MapType::new();
        for (vid, items) in vehicle_stacks {
            let plan: &mut Vec<VehicleRoute> = schedule.entry(vid).or_default();
            for item_id in items.into_iter() {
                let item = self.order_items.gets(&item_id);
                plan.push(VehicleRoute::new(
                    item.delivery_id.clone(),
                    VehicleWork::new_unload(&self.order_items, vec![item_id]),
                ));
            }
        }

        if allocate {
            let mut orders: MapType<OrderId, Vec<OrderItem>> = MapType::new();
            for (item_id, item) in items {
                if item_states.gets(&item_id) == &OrderItemState::Unallocated {
                    orders
                        .entry(item.id.order_id.clone())
                        .or_default()
                        .push(item);
                }
            }

            let vehicles: Vec<_> = self.vehicles.iter().map(|(_, v)| v).collect();
            let capacity = vehicles[0].capacity();
            let mut vehicle_idx = 0;

            for (_, items) in orders {
                let demand: i32 = items.iter().map(|i| i.demand).sum();
                if demand > capacity {
                    let mut cur_demand = 0;
                    let mut tmp_items = Vec::new();

                    for item in &items {
                        if cur_demand + item.demand > capacity {
                            let plan = schedule
                                .entry(vehicles[vehicle_idx].car_num.clone())
                                .or_default();
                            plan.push(VehicleRoute::new(
                                item.pickup_id.clone(),
                                VehicleWork::new_load(&self.order_items, tmp_items.clone()),
                            ));
                            plan.push(VehicleRoute::new(
                                item.delivery_id.clone(),
                                VehicleWork::new_unload(&self.order_items, tmp_items.clone()),
                            ));
                            cur_demand = 0;
                            tmp_items.clear();
                        }

                        vehicle_idx = (vehicle_idx + 1) % vehicles.len();
                        tmp_items.push(item.id.clone());
                        cur_demand += item.demand;
                    }

                    if !tmp_items.is_empty() {
                        let plan = schedule
                            .entry(vehicles[vehicle_idx].car_num.clone())
                            .or_default();
                        plan.push(VehicleRoute::new(
                            items[0].pickup_id.clone(),
                            VehicleWork::new_load(&self.order_items, tmp_items.clone()),
                        ));
                        plan.push(VehicleRoute::new(
                            items[0].delivery_id.clone(),
                            VehicleWork::new_unload(&self.order_items, tmp_items.clone()),
                        ));
                    }
                } else {
                    let plan = schedule
                        .entry(vehicles[vehicle_idx].car_num.clone())
                        .or_default();
                    let item_ids: Vec<_> = items.iter().map(|i| i.id.clone()).collect();
                    plan.push(VehicleRoute::new(
                        items.first().unwrap().pickup_id.clone(),
                        VehicleWork::new_load(&self.order_items, item_ids.clone()),
                    ));
                    plan.push(VehicleRoute::new(
                        items.first().unwrap().delivery_id.clone(),
                        VehicleWork::new_unload(&self.order_items, item_ids.clone()),
                    ));
                }

                vehicle_idx = (vehicle_idx + 1) % vehicles.len();
            }
        }

        deduplicate(&mut schedule);

        schedule
    }
}

impl Scheduler for NaiveScheduler {
    fn schedule(&mut self, args: SchedulerArgs) -> MapType<VehicleId, Vec<VehicleRoute>> {
        self.schedule_opt(args, true)
    }
}
