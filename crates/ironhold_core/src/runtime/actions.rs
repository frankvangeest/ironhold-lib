use bevy::prelude::*;

use crate::schema::Action;

#[derive(Resource, Default)]
pub struct ActionQueue(pub Vec<Action>);

impl ActionQueue {
    pub fn push(&mut self, action: Action) {
        self.0.push(action);
    }
    
    pub fn pop(&mut self) -> Option<Action> {
        self.0.pop()
    }
}
