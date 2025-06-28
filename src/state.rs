use gpui::*;
use crate::services::{SavedConnections, load_connections};

#[derive(Clone)]
pub struct AppState {
    pub saved_connections: Entity<SavedConnections>,
}

pub fn init(cx: &mut App) {
    let saved_connections = load_connections();
    println!("Found {} saved connections", saved_connections.len());
    let saved_connections_entity = cx.new(|_| SavedConnections {
        connections: saved_connections,
    });

    let app_state = AppState {
        saved_connections: saved_connections_entity
    };
    cx.set_global(app_state.clone());
}


impl Global for AppState {}
