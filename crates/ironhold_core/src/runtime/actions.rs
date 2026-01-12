use bevy::prelude::*;

#[derive(Debug, Clone)]
pub enum Action {
    LoadScene(String),
}

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
