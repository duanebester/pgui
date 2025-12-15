use gpui::*;

use crate::services::{DatabaseSchema, TableInfo};

pub struct EditorState {
    pub tables: Vec<TableInfo>,
    pub schema: Option<DatabaseSchema>,
}

impl Global for EditorState {}

impl EditorState {
    pub fn init(cx: &mut App) {
        let this = EditorState {
            tables: vec![],
            schema: None,
        };
        cx.set_global(this);
    }
}

pub struct EditorCodeActions {
    pub loading: bool,
}

impl Global for EditorCodeActions {}
impl EditorCodeActions {
    pub fn init(cx: &mut App) {
        let this = EditorCodeActions { loading: false };
        cx.set_global(this);
    }
}

pub struct EditorInlineCompletions {
    pub loading: bool,
}

impl Global for EditorInlineCompletions {}
impl EditorInlineCompletions {
    pub fn init(cx: &mut App) {
        let this = EditorInlineCompletions { loading: false };
        cx.set_global(this);
    }
}
