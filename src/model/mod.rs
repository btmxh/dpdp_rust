use std::{
    borrow::{Borrow, BorrowMut},
    collections::{BTreeMap, HashMap},
    hash::Hash,
    ops::RangeInclusive,
    path::Path,
};

use serde::{de::DeserializeOwned, Deserialize};

pub mod factory_info;
pub mod order;
pub mod order_item;
pub mod route_info;
pub mod vehicle_info;

static ALL_INSTANCES: RangeInclusive<i32> = 1..=64;

fn read_csv<T>(path: impl AsRef<Path>) -> anyhow::Result<Vec<T>>
where
    T: DeserializeOwned,
{
    let mut reader = csv::Reader::from_path(path)?;
    let records: csv::Result<Vec<T>> = reader.deserialize().collect();
    Ok(records?)
}

fn parse_naive_time<'de, D>(deserializer: D) -> Result<chrono::NaiveTime, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    chrono::NaiveTime::parse_from_str(&s, "%H:%M:%S").map_err(serde::de::Error::custom)
}

fn parse_duration<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = i64::deserialize(deserializer)?;
    Ok(chrono::Duration::seconds(s))
}

pub trait Map<K, V>: BorrowMut<MapType<K, V>> + Into<MapType<K, V>>
where
    K: Eq + Ord + 'static,
    V: 'static,
{
    fn gets<Q>(&self, key: &Q) -> &V
    where
        K: Borrow<Q> + Ord,
        Q: ?Sized + Hash + Eq + Ord,
    {
        self.borrow().get(key).expect("unchecked get failed")
    }

    fn gets_mut<Q>(&mut self, key: &Q) -> &mut V
    where
        K: Borrow<Q> + Ord,
        Q: ?Sized + Hash + Eq + Ord,
    {
        self.borrow_mut()
            .get_mut(key)
            .expect("unchecked get_mut failed")
    }

    fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: ?Sized + Hash + Eq + Ord,
    {
        self.borrow().get(key)
    }

    fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q> + Ord,
        Q: ?Sized + Hash + Eq + Ord,
    {
        self.borrow_mut().get_mut(key)
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.borrow().iter()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.borrow_mut().iter_mut()
    }

    fn keys(&self) -> impl Iterator<Item = &K> {
        self.borrow().keys()
    }

    fn values(&self) -> impl Iterator<Item = &V> {
        self.borrow().values()
    }

    fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.borrow_mut().values_mut()
    }

    fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q> + Ord,
        Q: ?Sized + Hash + Eq + Ord,
    {
        self.borrow().contains_key(key)
    }

    fn insert(&mut self, key: K, value: V) {
        self.borrow_mut().insert(key, value);
    }
}

pub type MapType<K, V> = BTreeMap<K, V>;

#[macro_export]
macro_rules! define_map {
    ($key:ty, $value:ty, $base:ident) => {
        #[derive(Debug, Default, Clone)]
        pub struct $base($crate::model::MapType<$key, $value>);

        impl std::borrow::Borrow<$crate::model::MapType<$key, $value>> for $base {
            fn borrow(&self) -> &$crate::model::MapType<$key, $value> {
                &self.0
            }
        }

        impl std::borrow::BorrowMut<$crate::model::MapType<$key, $value>> for $base {
            fn borrow_mut(&mut self) -> &mut $crate::model::MapType<$key, $value> {
                &mut self.0
            }
        }

        impl $crate::model::Map<$key, $value> for $base {}

        impl From<$crate::model::MapType<$key, $value>> for $base {
            fn from(map: $crate::model::MapType<$key, $value>) -> Self {
                Self(map)
            }
        }

        impl From<$base> for $crate::model::MapType<$key, $value> {
            fn from(base: $base) -> Self {
                base.0
            }
        }

        impl IntoIterator for $base {
            type Item = ($key, $value);
            type IntoIter =
                <$crate::model::MapType<$key, $value> as std::iter::IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.0.into_iter()
            }
        }
    };
}
