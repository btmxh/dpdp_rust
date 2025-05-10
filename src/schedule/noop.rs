use crate::{
    model::{vehicle_info::VehicleId, MapType},
    simulation::simulator::VehicleRoute,
};

use super::Scheduler;

#[derive(Debug, Default)]
pub struct NoopScheduler;

impl Scheduler for NoopScheduler {
    fn schedule(&mut self, _: super::SchedulerArgs) -> MapType<VehicleId, Vec<VehicleRoute>> {
        Default::default()
    }
}
