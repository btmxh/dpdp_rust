use core::f32;
use std::{collections::HashMap, path::Path};

use chrono::Duration;
use serde::Deserialize;

use super::{factory_info::FactoryId, read_csv, MapType};

#[derive(Debug, Clone, Deserialize)]
pub struct RouteInfo {
    route_code: String,
    start_factory_id: FactoryId,
    end_factory_id: FactoryId,
    distance: f32,
    time: i64,
}

impl RouteInfo {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Vec<RouteInfo>> {
        read_csv(path)
    }

    pub fn load_std() -> anyhow::Result<Vec<RouteInfo>> {
        Self::load("data/benchmark/route_info.csv")
    }
}

pub struct SingleRoute {
    route_code: String,
    distance: f32,
    time: i64,
}

pub struct RouteMap {
    map: MapType<(FactoryId, FactoryId), SingleRoute>,
}

impl From<Vec<RouteInfo>> for RouteMap {
    fn from(value: Vec<RouteInfo>) -> Self {
        let mut map = MapType::new();
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
    pub fn query_time(&self, from: FactoryId, to: FactoryId) -> Duration {
        if from == to {
            return Duration::zero();
        }
        self.map
            .get(&(from, to))
            .map(|r| r.time)
            .map(Duration::seconds)
            .unwrap_or(Duration::MAX)
    }

    pub fn query_distance(&self, from: FactoryId, to: FactoryId) -> f32 {
        if from == to {
            return 0.0;
        }
        self.map
            .get(&(from, to))
            .map(|r| r.distance)
            .unwrap_or(f32::MAX)
    }
}

#[test]
fn test_load_route_info() {
    assert!(RouteInfo::load_std().is_ok());
}
