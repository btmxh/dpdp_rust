use std::collections::BTreeMap;

use dyn_clone::DynClone;

use crate::{model::vehicle_info::VehicleId, schedule::SchedulerArgs};

use super::simulator::{SimEvent, VehicleRoute};

pub trait SimulationCallback: DynClone {
    fn visit_event(&mut self, event: &SimEvent) {}
    fn visit_dispatch_input(&mut self, input: &SchedulerArgs) {}
    fn visit_dispatch_output(&mut self, output: &BTreeMap<VehicleId, Vec<VehicleRoute>>) {}
}

dyn_clone::clone_trait_object!(SimulationCallback);
