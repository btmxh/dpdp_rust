use anyhow::{anyhow, Context as _};
use humantime::format_duration;
use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    time::Instant,
};

use chrono::{Duration, Local, NaiveDate, NaiveDateTime, NaiveTime};
use rand::{rngs::SmallRng, seq::IndexedRandom, Rng};

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
    schedule::{naive::NaiveScheduler, noop::NoopScheduler, Scheduler, SchedulerArgs},
};

use super::{
    callback::SimulationCallback,
    event_queue::EventQueue,
    sim_event::{SimulatorEventData, VehicleWork},
};

#[derive(Debug, Clone, Serialize)]
pub struct VehicleRoute {
    pub destination: FactoryId,
    pub work: VehicleWork,
}

impl VehicleRoute {
    pub fn new(destination: FactoryId, work: VehicleWork) -> Self {
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

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
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
    Delivered {
        deadline: NaiveDateTime,
        deliver_time: NaiveDateTime,
    },
}

impl OrderItemState {
    pub fn delivered(from: NaiveDateTime, to: NaiveDateTime) -> Self {
        Self::Delivered {
            deadline: from,
            deliver_time: to,
        }
    }

    fn delivered_irrelevant() -> Self {
        Self::delivered(NaiveDateTime::MAX, NaiveDateTime::MAX)
    }

    pub fn timeout(&self) -> Duration {
        match self {
            Self::Delivered {
                deadline,
                deliver_time,
            } => deliver_time.signed_duration_since(*deadline),
            _ => Duration::zero(),
        }
    }
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
    queue: VecDeque<(VehicleId, VehicleWork)>,
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

pub type SimEvent = (SimulatorEventData, NaiveDateTime);

pub struct Simulator {
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

    events: EventQueue<SimEvent>,

    total_distance: f32,
    total_distance_last_timeslot: f32,
    callbacks: Vec<Box<dyn SimulationCallback>>,
}

pub enum VehicleInitialPosition<'a, RNG = SmallRng> {
    Deterministic(MapType<VehicleId, FactoryId>),
    Random(&'a mut RNG),
}

impl<RNG: Rng> VehicleInitialPosition<'_, RNG> {
    pub fn get(&mut self, vehicle_id: &VehicleId, factories: &[FactoryId]) -> FactoryId {
        match self {
            Self::Deterministic(map) => map.get(vehicle_id).unwrap().clone(),
            Self::Random(rng) => factories.choose(rng).unwrap().clone(),
        }
    }
}

impl Simulator {
    pub fn new<RNG: Rng>(
        mut initial_position: VehicleInitialPosition<'_, RNG>,
        inst_num: i32,
        callbacks: Vec<Box<dyn SimulationCallback>>,
    ) -> anyhow::Result<Self> {
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
        let initial_date = Local::now().date_naive();
        let vehicle_states = vehicles
            .keys()
            .map(|id| {
                let init_pos = initial_position.get(id, &factory_ids);
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

        let time_interval = Duration::minutes(100);
        events.push((
            SimulatorEventData::UpdateTimestep,
            initial_date.and_time(NaiveTime::MIN),
        ));

        Ok(Self {
            routes: RouteInfo::load_std()
                .context("unable to load routes")?
                .into(),
            factories,
            vehicles,
            orders,
            order_items,

            initial_date,
            time_interval,

            vehicle_states,
            factory_states,
            order_item_states,

            events,
            scheduler: Box::new(
                NaiveScheduler::new(inst_num).context("unable to create scheduler")?,
            ),

            dock_approaching_time: Duration::minutes(30),
            total_distance: 0.0,
            total_distance_last_timeslot: 0.0,
            callbacks,
        })
    }

    fn group_order_item_ids<'a>(ids: impl Iterator<Item = &'a OrderItemId>) -> HashSet<OrderId> {
        ids.map(|id| id.order_id.clone()).collect()
    }

    pub fn simulate_until(&mut self, until: NaiveDateTime) {
        while self.events.peek().map(|e| e.1 <= until).unwrap_or(false) {
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
        let sim_event = (event_data, time);
        self.callbacks
            .iter_mut()
            .for_each(|cb| cb.visit_event(&sim_event));
        let (event_data, time) = sim_event;
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
            SimulatorEventData::FinishLoading {
                vehicle_id,
                factory_id,
                delivered_items,
            } => self.handle_finish_load(vehicle_id, factory_id, delivered_items, time),
            SimulatorEventData::UpdateTimestep => {
                self.handle_timestep(time);
            }
        }
    }

    fn begin_vehicle_loading(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        mut work: VehicleWork,
        time: NaiveDateTime,
    ) {
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::DoingWork(pos) if pos == &factory_id));
        let mut delivered_items = vec![];
        // ensure LIFO constraints
        while let Some(item) = work.unload_items.pop() {
            let corresponding_item = state.item_stack.pop();
            assert!(corresponding_item.as_ref() == Some(&item));
            delivered_items.push(item);
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
        assert!(total_demand <= self.vehicles.gets(&vehicle_id).capacity());
        let total_time = work.load_time + work.unload_time;
        self.events.push((
            SimulatorEventData::FinishLoading {
                vehicle_id,
                factory_id,
                delivered_items,
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
        println!("vehicle {vehicle_id} is following {route:?} at {time}");
        route.work.load_items.iter().for_each(|i| {
            *self.order_item_states.gets_mut(i) = OrderItemState::Allocated;
        });

        let total_time = self
            .routes
            .query_time(factory_id.clone(), route.destination.clone());
        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::Idle(pos) if pos == &factory_id));
        self.total_distance += self
            .routes
            .query_distance(factory_id.clone(), route.destination.clone());
        state.position = VehiclePosition::Transporting(factory_id, route.destination.clone());

        // simulate loading and unloading ahead of time
        for unload_item in route.work.unload_items.iter().rev() {
            let item = state.allocated_item_stack.pop();
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

    fn total_demand(&self, items: &[OrderItemId]) -> i32 {
        items.iter().map(|i| self.order_items.gets(i).demand).sum()
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

            let mut total_demand = self.total_demand(&state.allocated_item_stack);
            let mut item_stack = state.allocated_item_stack.clone();
            assert!(total_demand <= info.capacity());
            let mut item_states = self.order_item_states.clone();
            for route in routes {
                total_demand += route.delta_demand(&self.order_items);
                if total_demand > info.capacity() {
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

                    *item_state = OrderItemState::delivered_irrelevant();
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

                self.check_order_split(&route.work.load_items, info.capacity())?;
                self.check_order_split(&route.work.unload_items, info.capacity())?;
            }
        }

        Ok(())
    }

    fn handle_timestep(&mut self, time: NaiveDateTime) {
        let distance_travelled = self.total_distance - self.total_distance_last_timeslot;

        self.total_distance_last_timeslot = self.total_distance;
        let order_items = self
            .order_item_states
            .iter()
            .filter(|(_, state)| state != &&OrderItemState::Unavailable)
            .map(|(id, _)| (id.clone(), self.order_items.gets(id).clone()))
            .collect::<MapType<_, _>>();
        let vehicle_stacks = self
            .vehicle_states
            .iter()
            .map(|(id, state)| (id.clone(), state.allocated_item_stack.clone()))
            .collect::<MapType<_, _>>();
        let vehicle_positions = self
            .vehicle_states
            .iter()
            .map(|(id, state)| (id.clone(), state.position.clone()))
            .collect::<MapType<_, _>>();

        let start = Instant::now();
        let sim = self.fork(Box::new(NoopScheduler), Some(time));
        // let args = SchedulerArgs::new(sim);
        let args = SchedulerArgs {
            items: order_items.into(),
            item_states: self.order_item_states.clone(),
            vehicle_stacks,
            vehicle_positions,
            time,
            elapsed_distance: distance_travelled,
            static_simulator: sim,
        };
        self.callbacks
            .iter_mut()
            .for_each(|cb| cb.visit_dispatch_input(&args));
        let planned_routes = self.scheduler.schedule(args);
        self.callbacks
            .iter_mut()
            .for_each(|cb| cb.visit_dispatch_output(&planned_routes));
        println!("planned route: {:?}", planned_routes);

        let schedule_time = start.elapsed();
        let intervals =
            1 + (schedule_time.as_nanos() / self.time_interval.to_std().unwrap().as_nanos()) as i32;
        println!(
            "scheduling time: {} ({} intervals)",
            format_duration(schedule_time),
            intervals
        );

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

        if let Some((item, _)) = self
            .order_item_states
            .iter()
            .find(|(_, s)| !matches!(s, OrderItemState::Delivered { .. }))
        {
            println!("{item} is not delivered yet, continuing simulation");
            self.events.push((
                SimulatorEventData::UpdateTimestep,
                time + self.time_interval * intervals,
            ));
        } else {
            let mut order_timeouts: MapType<OrderId, Duration> = Default::default();
            let mut order_deliver_times: MapType<OrderId, NaiveDateTime> = Default::default();
            for (item, state) in self.order_item_states.iter() {
                let timeout = order_timeouts
                    .entry(item.order_id.clone())
                    .or_insert(Duration::MIN);
                let max_timeout = (*timeout).max(state.timeout());
                *timeout = max_timeout;
                let order_deliver_time = order_deliver_times
                    .entry(item.order_id.clone())
                    .or_insert(NaiveDateTime::MIN);
                if let OrderItemState::Delivered { deliver_time, .. } = state {
                    let max_deliver_time = (*order_deliver_time).max(*deliver_time);
                    *order_deliver_time = max_deliver_time;
                }
            }
            let total_timeout: Duration = order_timeouts
                .values()
                .map(|t| (*t).max(Duration::zero()))
                .sum();
            let total_timeout_str = format_duration(total_timeout.to_std().unwrap());
            let total_distance = self.total_distance;
            for (order_id, timeout) in order_timeouts {
                let deliver_time = order_deliver_times
                    .get(&order_id)
                    .unwrap()
                    .and_local_timezone(Local)
                    .unwrap()
                    .timestamp();
                let deadline = self
                    .orders
                    .gets(&order_id)
                    .committed_completion_time(self.initial_date)
                    .and_local_timezone(Local)
                    .unwrap()
                    .timestamp();
                println!("{order_id} timeout: {timeout} ({deliver_time} - {deadline})");
            }
            println!(
                "all items are delivered, total timeout {total_timeout_str} ({total_timeout}), total distance {total_distance}"
            );
        }
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
        work: VehicleWork,
        time: NaiveDateTime,
    ) {
        let state = self.vehicle_states.gets_mut(&vehicle_id);
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
        work: VehicleWork,
        time: NaiveDateTime,
    ) {
        let state = self.factory_states.gets_mut(&factory_id);
        if state.num_avail_docks == 0 {
            println!("factory {factory_id} is full, waiting...");
            state.queue.push_back((vehicle_id, work));
        } else {
            state.num_avail_docks -= 1;
            self.begin_vehicle_loading(vehicle_id, factory_id, work, time);
        }
    }

    fn handle_finish_load(
        &mut self,
        vehicle_id: VehicleId,
        factory_id: FactoryId,
        delivered_items: Vec<OrderItemId>,
        time: NaiveDateTime,
    ) {
        let factory = self.factory_states.gets_mut(&factory_id);
        if let Some((vehicle_id, work)) = factory.queue.pop_front() {
            self.begin_vehicle_loading(vehicle_id, factory_id.clone(), work, time);
        } else {
            factory.num_avail_docks += 1;
        }

        println!("{delivered_items:?} are delivered");
        let unload_time: Duration = delivered_items
            .iter()
            .map(|id| self.order_items.gets(id).unload_time)
            .sum();

        for item in delivered_items.iter() {
            let item_info = self.order_items.gets(item);
            *self.order_item_states.gets_mut(item) = OrderItemState::delivered(
                item_info.committed_completion_time(self.initial_date),
                time - self.dock_approaching_time - unload_time,
            );
        }

        let state = self.vehicle_states.gets_mut(&vehicle_id);
        assert!(matches!(&state.position, VehiclePosition::DoingWork(pos) if pos == &factory_id));
        state.position = VehiclePosition::Idle(factory_id.clone());

        if let Some(dest) = state.current_route.pop_front() {
            self.begin_vehicle_transporting(vehicle_id, factory_id, dest, time);
        }
    }

    pub fn fork(
        &self,
        scheduler: Box<dyn Scheduler>,
        static_deadline: Option<NaiveDateTime>,
    ) -> Self {
        let mut orders = self.orders.clone();
        let mut order_items = self.order_items.clone();

        if let Some(static_deadline) = static_deadline {
            orders.retain(|_, order| {
                self.initial_date.and_time(order.creation_time) <= static_deadline
            });
            order_items.retain(|_, item| {
                self.initial_date.and_time(item.creation_time) <= static_deadline
            });
        }

        Self {
            routes: self.routes.clone(),
            factories: self.factories.clone(),
            vehicles: self.vehicles.clone(),
            orders,
            order_items,
            initial_date: self.initial_date.clone(),
            time_interval: self.time_interval.clone(),
            vehicle_states: self.vehicle_states.clone(),
            factory_states: self.factory_states.clone(),
            order_item_states: self.order_item_states.clone(),
            dock_approaching_time: self.dock_approaching_time.clone(),
            scheduler,
            events: self.events.clone(),
            total_distance: self.total_distance.clone(),
            total_distance_last_timeslot: self.total_distance_last_timeslot.clone(),
            callbacks: self.callbacks.clone(),
        }
    }
}
