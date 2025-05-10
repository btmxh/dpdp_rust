use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    callbacks::dump_json,
    model::vehicle_info::VehicleId,
    schedule::SchedulerArgs,
    simulation::{callback::SimulationCallback, simulator::VehicleRoute},
};

pub struct LogDispatchCallback {
    name: String,
    iteration: usize,
}

impl LogDispatchCallback {
    pub fn new(name: String) -> Self {
        Self { name, iteration: 0 }
    }

    pub fn get_file(&self, filename: &str) -> PathBuf {
        let mut dir = PathBuf::new();
        dir.push("logs");
        dir.push(&self.name);
        dir.push(format!("{}", self.iteration));
        dir.push(filename);
        dir
    }
}

impl Clone for LogDispatchCallback {
    fn clone(&self) -> Self {
        Self {
            name: format!("{}_cloned", self.name),
            iteration: self.iteration,
        }
    }
}

impl SimulationCallback for LogDispatchCallback {
    fn visit_dispatch_input(&mut self, input: &SchedulerArgs) {
        if let Err(err) = dump_json(self.get_file("dispatch_input.json"), &input) {
            eprintln!("Failed to write dispatch input JSON file: {}", err);
        }
    }

    fn visit_dispatch_output(&mut self, output: &BTreeMap<VehicleId, Vec<VehicleRoute>>) {
        if let Err(err) = dump_json(self.get_file("dispatch_output.json"), output) {
            eprintln!("Failed to write dispatch output JSON file: {}", err);
        }
        self.iteration += 1
    }
}
