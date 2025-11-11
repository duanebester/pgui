use gpui::*;
use gpui_component::{
    IndexPath,
    list::{List, ListDelegate},
};

use crate::{services::*, workspace::connections::ConnectionListItem};

pub struct ConnectionListDelegate {
    connections: Vec<ConnectionInfo>,
    pub matched_connections: Vec<ConnectionInfo>,
    selected_index: Option<IndexPath>,
    query: String,
}

impl ListDelegate for ConnectionListDelegate {
    type Item = ConnectionListItem;

    fn items_count(&self, _section: usize, _app: &App) -> usize {
        self.matched_connections.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Task<()> {
        self.query = query.to_string();
        self.matched_connections = if query.is_empty() {
            self.connections.clone()
        } else {
            self.connections
                .iter()
                .filter(|conn| {
                    conn.database.to_lowercase().contains(&query.to_lowercase())
                        || conn.username.to_lowercase().contains(&query.to_lowercase())
                })
                .cloned()
                .collect()
        };
        Task::ready(())
    }

    fn confirm(&mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<List<Self>>) {
        if let Some(selected) = self.selected_index {
            if let Some(conn) = self.matched_connections.get(selected.row) {
                println!("Selected connection: {}@{}", conn.username, conn.hostname);
            }
        }
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<List<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<List<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        if let Some(conn) = self.matched_connections.get(ix.row) {
            return Some(ConnectionListItem::new(ix, conn.clone(), ix, selected));
        }
        None
    }
}

impl ConnectionListDelegate {
    pub fn new() -> Self {
        Self {
            connections: vec![],
            matched_connections: vec![],
            selected_index: None,
            query: String::new(),
        }
    }

    pub fn update_connections(&mut self, connections: Vec<ConnectionInfo>) {
        self.connections = connections;
        self.matched_connections = self.connections.clone();
        if !self.matched_connections.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(IndexPath::default());
        }
    }
}
