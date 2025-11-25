use gpui::*;

use crate::services::DatabaseInfo;

pub struct DatabaseState {
    pub databases: Vec<DatabaseInfo>,
}

impl Global for DatabaseState {}

impl DatabaseState {
    pub fn init(cx: &mut App) {
        let this = DatabaseState { databases: vec![] };
        cx.set_global(this);
    }
}
