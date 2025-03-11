use anyhow::{anyhow, Context as _};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rand::{seq::IndexedRandom, Rng};

use crate::{
    define_map,
    model::{
        factory_info::{FactoryId, FactoryInfo, FactoryInfoMap},
        order::{Order, OrderId, OrderMap},
        order_item::{OrderItemId, OrderItemMap},
        route_info::{RouteInfo, RouteMap},
        vehicle_info::{VehicleId, VehicleInfo, VehicleInfoMap},
        Map, MapType,
    },
    schedule::{naive::NaiveScheduler, Scheduler},
};

use super::{
    event_queue::EventQueue,
    sim_event::{LoadUnloadWork, SimulatorEventData},
};

#[derive(Debug, Clone)]
pub struct VehicleRoute {
    pub destination: FactoryId,
    pub work: LoadUnloadWork,
}

impl VehicleRoute {
    pub fn new(destination: FactoryId, work: LoadUnloadWork) -> Self {
        Self { destination, work }
    }

    pub fn delta_demand(&self, order_items: &OrderItemMap) -> i32 {
        self.work.delta_demand(order_items)
    }

    pub fn try_merge(&mut self, route: VehicleRoute) -> Result<(), VehicleRoute> {
        if self.destination == route.destination {
            self.work.merge(route.work);
            Ok(())
        } else {
            Err(route)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VehiclePosition {
    Idle(FactoryId),
    DoingWork(FactoryId),
    Transporting(FactoryId, FactoryId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderItemState {
    // now < creation_time
    Unavailable,
    // available but not assigned to any vehicle
    Unallocated,
    // assigned to a vehicle, yet to be pick up
    Allocated,
    // picked up by a vehicle, yet to be delivered
    PickedUp,
    // delivered
    Delivered,
}

#[derive(Debug, Clone)]
pub struct VehicleState {
    position: VehiclePosition,
    item_stack: Vec<OrderItemId>,
    allocated_item_stack: Vec<OrderItemId>,
    // planning information
    current_route: VecDeque<VehicleRoute>,
}

#[derive(Debug, Clone)]
pub struct FactoryState {
    num_avail_docks: i32,
    queue: VecDeque<(VehicleId, LoadUnloadWork)>,
}

impl FactoryState {
    pub fn new(num_avail_docks: i32) -> Self {
        Self {
            num_avail_docks,
            queue: VecDeque::new(),
        }
    }
}

define_map!(FactoryId, FactoryState, FactoryStateMap);
define_map!(VehicleId, VehicleState, VehicleStateMap);
define_map!(OrderItemId, OrderItemState, OrderItemStateMap);

impl VehicleState {
    pub fn new(factory_id: FactoryId) -> Self {
        Self {
            position: VehiclePosition::Idle(factory_id),
            item_stack: Vec::new(),
            allocated_item_stack: Vec::new(),
            current_route: VecDeque::new(),
        }
    }
}

pub struct Simulator<RNG> {
    rng: RefCell<RNG>,

    routes: RouteMap,
    factories: FactoryInfoMap,
    vehicles: VehicleInfoMap,
    orders: OrderMap,
    order_items: OrderItemMap,

    initial_date: NaiveDate,
    time_interval: Duration,

    vehicle_states: VehicleStateMap,
    factory_states: FactoryStateMap,
    order_item_states: OrderItemStateMap,

    dock_approaching_time: Duration,

    scheduler: Box<dyn Scheduler>,

    events: EventQueue<(SimulatorEventData, NaiveDateTime)>,
}

impl<RNG: Rng> Simulator<RNG> {
    pub fn new(mut rng: RNG, inst_num: i32) -> anyhow::Result<Self> {
        let orders = Order::load_instance(inst_num).context("unable to load orders")?;
        let order_items: OrderItemMap = orders
            .values()
            .flat_map(Order::into_items)
            .map(|o| (o.id.clone(), o))
            .collect::<MapType<_, _>>()
            .into();
        let vehicles = VehicleInfo::load_instance(inst_num).context("unable to load vehicles")?;
        let factories = FactoryInfo::load_std().context("unable to load factories")?;
        let factory_ids: Vec<_> = factories.keys().cloned().collect();
        let initial_date = Utc::now().date_naive();
        let vehicle_states = vehicles
            .keys()
            .map(|id| {
                let init_pos = factory_ids
                    .choose(&mut rng)
                    .expect("should be present")
                    .clone();
                (id.clone(), VehicleState::new(init_pos))
            })
            .collect::<MapType<_, _>>()
            .into();
        let factory_states = factories
            .iter()
            .map(|(id, info)| (id.clone(), FactoryState::new(info.port_num)))
            .collect::<MapType<_, _>>()
            .into();
        let order_item_states = order_items
            .keys()
            .map(|id| (id.clone(), OrderItemState::Unavailable))
            .collect::<MapType<_, _>>()
            .into();

        let mut events = EventQueue::new();
        for order in orders.values() {
            events.push((
                SimulatorEventData::OrderArrival {
                    order_id: order.order_id.clone(),
                    order_item_ids: order.into_items().into_iter().map(|o| o.id).collect(),
                },
                initial_date.and_time(order.creation_time),
            ));
        }

        events.push((
            SimulatorEventData::UpdateTimestep,
            initial_date.and_time(NaiveTime::MIN),
        ));

        Ok(Self {
            rng: RefCell::new(rng),

            routes: RouteInfo::load_std()
                .context("unable to load routes")?
                .into(),
            factories,
            vehicles,
            orders,
            order_items,

            initial_date,
            time_interval: Duration::minutes(10),

            vehicle_states,
            factory_states,
            order_item_states,

            events,
            scheduler: Box::new(
                NaiveScheduler::new(inst_num).context("unable to create scheduler")?,
            ),

            dock_approaching_time: Duration::minutes(30),
        })
    }

    fn group_order_item_ids<'a>(ids: impl Iterator<Item = &'a OrderItemId>) -> HashSet<OrderId> {
        ids.map(|id| id.order_id.clone()).collect()
    }

    pub fn simulate_until(&mut self, until: NaiveDateTime) {
        while self.events.peek().map(|e| e.1 < until).unwrap_or(false) {
            self.simulate_step();
        }
    }

    pub fn simulate_step(&mut self) {
        if let Some((event, time)) = self.events.pop() {
            self.handle_event(event, time);
        }
    }

    fn handle_event(&mut self, event_data: SimulatorEventData, time: NaiveDateTime) {
        println!("handling event {event_data:?} at {time}");
        match event_data {
            SimulatorEventData::OrderArrival {
                order_id,
                order_item_ids,
            } => self.handle_order_arrival(order_id, order_item_ids, time),
            SimulatorEventData::VehicleArrival {
                vehicle_id,
                factory_id,
                work,
            } => self.handle_vehicle_arrival(vehicle_id, factory_id, work, time),
            SimulatorEventData::VehicleApproachedDock {
                vehicle_id,
                factory_id,
                work,
            } => self.handle_vehicle_approached_dock(vehicle_id, factory_id, work, time),
            SimulatorEventData::DockAvailable { factory_id } => {
                self.handle_dock_available(factory_id, time)
            }
            SimulatorEventData::FinishLoading {
                vehicle_id,
                factory_id,
            } => self.handle_finish_load(vehicle_id, factory_id, time),
            SimulatorEventData::UpdateTimestep => {
                self.handle_timestep(time);
            }
        }
    }

    fn begin_vehicle_loading(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        mut work: LoadUnloadWork,
        time: NaiveDateTime,
    ) {
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::DoingWork(pos) if pos == &factory_id));
        // ensure LIFO constraints
        while let Some(item) = work.unload_items.pop() {
            let corresponding_item = state.item_stack.pop();
            *self.order_item_states.gets_mut(&item) = OrderItemState::Delivered;
            assert!(corresponding_item == Some(item));
        }
        for item in work.load_items.iter() {
            *self.order_item_states.gets_mut(item) = OrderItemState::PickedUp;
        }
        state.item_stack.extend(work.load_items);
        let total_demand: i32 = state
            .item_stack
            .iter()
            .map(|i| self.order_items.gets(i).demand)
            .sum();
        // ensure capacity constraints
        assert!(total_demand <= self.vehicles.gets(&vehicle_id).capacity);
        let total_time = work.load_time + work.unload_time;
        self.events.push((
            SimulatorEventData::FinishLoading {
                vehicle_id,
                factory_id,
            },
            time + total_time,
        ));
    }

    fn begin_vehicle_transporting(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        route: VehicleRoute,
        time: NaiveDateTime,
    ) {
        println!("vehicle {vehicle_id} is following {route:?}");
        route.work.load_items.iter().for_each(|i| {
            *self.order_item_states.gets_mut(i) = OrderItemState::Allocated;
        });

        let total_time = self
            .routes
            .query_time(factory_id.clone(), route.destination.clone());
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::Idle(pos) if pos == &factory_id));
        state.position = VehiclePosition::Transporting(factory_id, route.destination.clone());

        // simulate loading and unloading ahead of time
        for unload_item in route.work.unload_items.iter().rev() {
            let item = state.allocated_item_stack.pop();
            println!("{item:?} {unload_item:?}");
            assert!(item.as_ref() == Some(unload_item));
        }
        state
            .allocated_item_stack
            .extend(route.work.load_items.clone());

        self.events.push((
            SimulatorEventData::VehicleArrival {
                vehicle_id,
                factory_id: route.destination,
                work: route.work,
            },
            time + total_time,
        ));
    }

    fn total_demand(&self, state: &VehicleState) -> i32 {
        state
            .item_stack
            .iter()
            .map(|i| self.order_items.gets(i).demand)
            .sum()
    }

    fn check_order_split(&self, item_ids: &[OrderItemId], capacity: i32) -> anyhow::Result<()> {
        let orders: HashSet<OrderId> = item_ids.iter().map(|item| item.order_id.clone()).collect();

        for order_id in orders {
            let order = self
                .orders
                .get(&order_id)
                .ok_or_else(|| anyhow!("Invalid order ID: {}", order_id))?;
            if order.calc_demand() <= capacity {
                let item_set: HashSet<OrderItemId> = item_ids.iter().cloned().collect();
                if order
                    .into_items()
                    .iter()
                    .any(|item| !item_set.contains(&item.id))
                {
                    return Err(anyhow!(
                                "Order {} has demand {} < capacity {} is split (orders can only be split if the demand exceeds vehicle capacity)",
                                order_id, order.calc_demand(), capacity
                            ));
                }
            }
        }

        Ok(())
    }

    fn check_planned_routes(
        &self,
        planned_routes: &MapType<VehicleId, Vec<VehicleRoute>>,
    ) -> anyhow::Result<()> {
        for (vehicle_id, routes) in planned_routes {
            let info = self
                .vehicles
                .get(vehicle_id)
                .ok_or_else(|| anyhow!("Invalid vehicle ID: {}", vehicle_id))?;
            let state = self
                .vehicle_states
                .get(vehicle_id)
                .ok_or_else(|| anyhow!("Invalid vehicle ID: {}", vehicle_id))?;

            let mut total_demand = self.total_demand(state);
            let mut item_stack = state.allocated_item_stack.clone();
            assert!(total_demand <= info.capacity);
            let mut item_states = self.order_item_states.clone();
            for route in routes {
                total_demand += route.delta_demand(&self.order_items);
                if total_demand > info.capacity {
                    return Err(anyhow!(
                        "Violate capacity constraint on vehicle {}!",
                        vehicle_id
                    ));
                }

                for item in route.work.unload_items.iter().rev() {
                    if item_stack.pop().as_ref() != Some(item) {
                        return Err(anyhow!(
                            "Violate LIFO constraint on vehicle {}!",
                            vehicle_id
                        ));
                    }

                    let item_info = self
                        .order_items
                        .get(item)
                        .ok_or_else(|| anyhow!("Invalid order item ID: {}", item))?;
                    if item_info.delivery_id != route.destination {
                        return Err(anyhow!(
                            "Order item {} delivery location is {}, not {}!",
                            item,
                            item_info.delivery_id,
                            route.destination
                        ));
                    }

                    let item_state = item_states
                        .get_mut(item)
                        .ok_or_else(|| anyhow!("Invalid order item ID: {}", item))?;
                    if *item_state != OrderItemState::Allocated
                        && *item_state != OrderItemState::PickedUp
                    {
                        return Err(anyhow!(
                            "Order item {} is scheduled to be unloaded, but it is not allocated or picked up yet!",
                            item
                        ));
                    }

                    *item_state = OrderItemState::Delivered;
                }

                item_stack.reserve(route.work.load_items.len());
                for item in &route.work.load_items {
                    let item_info = self.order_items.gets(item);
                    if item_info.pickup_id != route.destination {
                        return Err(anyhow!(
                            "Order item {} pickup location is {}, not {}!",
                            item,
                            item_info.pickup_id,
                            route.destination
                        ));
                    }
                    item_stack.push(item.clone());

                    let item_state = item_states
                        .get_mut(item)
                        .ok_or_else(|| anyhow!("Invalid order item ID: {}", item))?;
                    if *item_state != OrderItemState::Unallocated {
                        return Err(anyhow!(
                            "Order item {} is scheduled to be loaded, but it is not unallocated yet!",
                            item
                        ));
                    }
                    *item_state = OrderItemState::PickedUp;
                }

                self.check_order_split(&route.work.load_items, info.capacity)?;
                self.check_order_split(&route.work.unload_items, info.capacity)?;
            }
        }

        Ok(())
    }

    fn handle_timestep(&mut self, time: NaiveDateTime) {
        let unallocated_order_items = self
            .order_item_states
            .iter()
            .filter(|(_, state)| state == &&OrderItemState::Unallocated)
            .map(|(id, _)| (id.clone(), self.order_items.gets(id).clone()))
            .collect::<MapType<_, _>>();
        let ongoing_order_items = self
            .order_item_states
            .iter()
            .filter(|(_, state)| {
                state != &&OrderItemState::Unallocated && state != &&OrderItemState::Delivered
            })
            .map(|(id, _)| (id.clone(), self.order_items.gets(id).clone()))
            .collect::<MapType<_, _>>();
        let vehicle_stacks = self
            .vehicle_states
            .iter()
            .map(|(id, state)| (id.clone(), state.allocated_item_stack.clone()))
            .collect::<MapType<_, _>>();

        // println!("unallocated order items: {unallocated_order_items:#?}");
        // println!("ongoing order items: {ongoing_order_items:#?}");
        println!("vehicle stacks: {vehicle_stacks:#?}");
        let planned_routes = self.scheduler.schedule(
            unallocated_order_items.into(),
            ongoing_order_items.into(),
            vehicle_stacks,
            time,
        );

        println!("planned routes: {planned_routes:#?}");

        if let Err(err) = self.check_planned_routes(&planned_routes) {
            panic!("invalid planning routes: {}", err);
        }

        for (vehicle_id, routes) in planned_routes {
            let state = self.vehicle_states.gets_mut(&vehicle_id);
            state.current_route.clear();
            state.current_route.extend(routes);

            if let VehiclePosition::Idle(start) = state.position.clone() {
                if let Some(dest) = state.current_route.pop_front() {
                    self.begin_vehicle_transporting(vehicle_id, start.clone(), dest, time);
                }
            }
        }

        self.events.push((
            SimulatorEventData::UpdateTimestep,
            time + self.time_interval,
        ));
    }

    fn handle_order_arrival(
        &mut self,
        _order_id: OrderId,
        order_item_ids: Vec<OrderItemId>,
        _time: NaiveDateTime,
    ) {
        for id in order_item_ids {
            *self.order_item_states.gets_mut(&id) = OrderItemState::Unallocated;
        }
    }

    fn handle_vehicle_arrival(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        work: LoadUnloadWork,
        time: NaiveDateTime,
    ) {
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        println!("current state of {vehicle_id} is {state:?}");
        assert!(
            matches!(&state.position, VehiclePosition::Transporting(_, dest) if dest == &factory_id)
        );
        state.position = VehiclePosition::DoingWork(factory_id.clone());

        self.events.push((
            SimulatorEventData::VehicleApproachedDock {
                vehicle_id,
                factory_id,
                work,
            },
            time + self.dock_approaching_time,
        ));
    }

    fn handle_vehicle_approached_dock(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        work: LoadUnloadWork,
        time: NaiveDateTime,
    ) {
        let state = self.factory_states.gets_mut(&factory_id);
        if state.num_avail_docks == 0 {
            state.queue.push_back((vehicle_id, work));
        } else {
            state.num_avail_docks -= 1;
            self.begin_vehicle_loading(vehicle_id, factory_id, work, time);
        }
    }

    fn handle_dock_available(&mut self, factory_id: FactoryId, time: NaiveDateTime) {
        let state = self.factory_states.gets_mut(&factory_id);
        if let Some((vehicle_id, work)) = state.queue.pop_front() {
            assert!(state.num_avail_docks == 0);
            self.begin_vehicle_loading(vehicle_id, factory_id, work, time);
        } else {
            state.num_avail_docks += 1;
        }
    }
    fn handle_finish_load(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        time: NaiveDateTime,
    ) {
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::DoingWork(pos) if pos == &factory_id));
        state.position = VehiclePosition::Idle(factory_id.clone());

        if let Some(dest) = state.current_route.pop_front() {
            self.begin_vehicle_transporting(vehicle_id, factory_id, dest, time);
        }
    }
}
