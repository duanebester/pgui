use std::collections::HashMap;
use std::rc::Rc;
use std::sync::LazyLock;

use gpui::*;
use gpui_component::Theme;
use gpui_component::ThemeConfig;
use gpui_component::ThemeMode;
use gpui_component::ThemeSet;

pub static THEMES: LazyLock<HashMap<SharedString, ThemeConfig>> = LazyLock::new(|| {
    fn parse_themes(source: &str) -> ThemeSet {
        serde_json::from_str(source).unwrap()
    }

    let mut themes = HashMap::new();
    for source in [include_str!("./catppuccin.json")] {
        let theme_set = parse_themes(source);
        for theme in theme_set.themes {
            themes.insert(theme.name.clone(), theme);
        }
    }

    themes
});

// Apply a Catppuccin theme by color mode
pub fn change_color_mode(mode: ThemeMode, _win: &mut Window, cx: &mut App) {
    let theme_name = match mode {
        ThemeMode::Light => "Catppuccin Latte",
        ThemeMode::Dark => "Catppuccin Macchiato",
    };

    if let Some(theme_config) = THEMES.get(theme_name) {
        let theme_config = Rc::new(theme_config.clone());
        let theme = Theme::global_mut(cx);
        theme.mode = theme_config.mode;
        theme.apply_config(&theme_config);
    }
}
