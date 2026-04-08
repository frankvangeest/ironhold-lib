use bevy::prelude::*;
use std::collections::VecDeque;

use crate::schema::Action;

/// First-in, first-out action queue.
///
/// Actions are dequeued in the order they were pushed — push order equals
/// execution order. The executor drains this queue each frame.
#[derive(Resource, Default)]
pub struct ActionQueue(pub VecDeque<Action>);

impl ActionQueue {
    pub fn push(&mut self, action: Action) {
        self.0.push_back(action);
    }

    pub fn pop(&mut self) -> Option<Action> {
        self.0.pop_front()
    }
}
