use core::f32;
use std::{collections::HashMap, path::Path};

use serde::Deserialize;

use super::read_csv;

#[derive(Debug, Deserialize)]
pub struct RouteInfo {
    route_code: String,
    start_factory_id: String,
    end_factory_id: String,
    distance: f32,
    time: i32,
}

impl RouteInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Vec<RouteInfo>> {
        read_csv(path)
    }
}

pub struct SingleRoute {
    route_code: String,
    distance: f32,
    time: i32,
}

pub struct RouteMap {
    map: HashMap<(String, String), SingleRoute>,
}

impl From<Vec<RouteInfo>> for RouteMap {
    fn from(value: Vec<RouteInfo>) -> Self {
        let mut map = HashMap::new();
        for r in value {
            map.insert(
                (r.start_factory_id, r.end_factory_id),
                SingleRoute {
                    route_code: r.route_code,
                    distance: r.distance,
                    time: r.time,
                },
            );
        }
        RouteMap { map }
    }
}

impl RouteMap {
    pub fn query_time(&self, from: String, to: String) -> i32 {
        self.map
            .get(&(from, to))
            .map(|r| r.time)
            .unwrap_or(i32::MAX)
    }

    pub fn query_distance(&self, from: String, to: String) -> f32 {
        self.map
            .get(&(from, to))
            .map(|r| r.distance)
            .unwrap_or(f32::INFINITY)
    }
}

#[test]
fn test_load_route_info() {
    assert!(RouteInfo::load("data/benchmark/route_info.csv").is_ok());
}
