use bevy::prelude::*;
use std::collections::VecDeque;

use crate::schema::Action;

/// First-in, first-out action queue.
///
/// Actions are dequeued in the order they were pushed — push order equals
/// execution order, with one executor-only exception (`push_front_all`, used by `Action::UseItem`).
/// The executor drains this queue each frame.
#[derive(Resource, Default)]
pub struct ActionQueue(pub VecDeque<Action>);

impl ActionQueue {
    pub fn push(&mut self, action: Action) {
        self.0.push_back(action);
    }

    pub fn pop(&mut self) -> Option<Action> {
        self.0.pop_front()
    }

    /// Inserts `actions` at the FRONT of the queue, keeping their order, so they run next, before anything already
    /// queued. **Executor-only, the one exception to "push order = execution order":** `Action::UseItem` uses it so an
    /// item's effects run right after the use that produced them instead of after the rest of the batch (a `LoadScene`
    /// queued behind the use must not run first). Interpreters and capability systems never call this, they `push`.
    pub(crate) fn push_front_all(&mut self, actions: Vec<Action>) {
        for action in actions.into_iter().rev() {
            self.0.push_front(action);
        }
    }
}
