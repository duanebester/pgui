use gpui::*;

use crate::services::TableInfo;

pub struct EditorState {
    pub tables: Vec<TableInfo>,
}

impl Global for EditorState {}

impl EditorState {
    pub fn init(cx: &mut App) {
        let this = EditorState { tables: vec![] };
        cx.set_global(this);
    }
}
