use std::{cmp::Reverse, collections::BinaryHeap};

use chrono::NaiveDateTime;

pub trait Event {
    fn time(&self) -> NaiveDateTime;

    fn time_rev(&self) -> Reverse<NaiveDateTime> {
        Reverse(self.time())
    }
}

#[derive(Debug, Clone)]
pub struct EventWrapper<E: Event>(E);

impl<E: Event> PartialEq for EventWrapper<E> {
    fn eq(&self, other: &Self) -> bool {
        self.0.time_rev() == other.0.time_rev()
    }
}

impl<E: Event> PartialOrd for EventWrapper<E> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<E: Event> Eq for EventWrapper<E> {}
impl<E: Event> Ord for EventWrapper<E> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.time_rev().cmp(&other.0.time_rev())
    }
}

#[derive(Debug, Clone)]
pub struct EventQueue<E: Event> {
    events: BinaryHeap<EventWrapper<E>>,
}

impl<E: Event> EventQueue<E> {
    pub fn new() -> EventQueue<E> {
        EventQueue {
            events: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: E) {
        self.events.push(EventWrapper(event));
    }

    pub fn pop(&mut self) -> Option<E> {
        self.events.pop().map(|EventWrapper(e)| e)
    }

    pub fn peek(&self) -> Option<&E> {
        self.events.peek().map(|EventWrapper(e)| e)
    }
}

impl<E: Event> Default for EventQueue<E> {
    fn default() -> Self {
        Self::new()
    }
}
