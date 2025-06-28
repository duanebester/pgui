mod assets;
mod services;
mod state;
mod themes;
mod window;
mod workspace;

use assets::Assets;
use themes::*;
use window::*;

use gpui::*;
use gpui_component::{
    highlighter::{self, LanguageRegistry}, theme, ActiveTheme as _, Root, Theme, ThemeMode
};
use workspace::Workspace;

/// Apply a Catppuccin theme by name
pub fn apply_catppuccin_theme(name: &str, win: &mut Window, cx: &mut App) {
    let (colors, mode) = match name.to_lowercase().as_str() {
        "latte" => (catppuccin_latte(), ThemeMode::Light),
        "macchiato" => (catppuccin_macchiato(), ThemeMode::Dark),
        _ => {
            eprintln!("Unknown Catppuccin theme: {}", name);
            return;
        }
    };

    let theme = cx.global_mut::<Theme>();
    theme.mode = mode;
    theme.colors = colors;

    let language_registry = cx.global_mut::<LanguageRegistry>();
    language_registry.set_theme(&DEFAULT_LIGHT.clone(), &DEFAULT_DARK.clone());

    win.refresh();
}

fn main() {
    let application = Application::new().with_assets(Assets);

    application.run(|cx: &mut App| {
        let window_options = get_window_options(cx);
        cx.open_window(window_options, |win, cx| {
            gpui_component::init(cx);
            highlighter::init(cx);
            theme::init(cx);
            state::init(cx);

            match cx.theme().mode.is_dark() {
                true => apply_catppuccin_theme("macchiato", win, cx),
                false => apply_catppuccin_theme("latte", win, cx),
            };

            let workspace_view = Workspace::view(win, cx);
            let drawer_layer = Root::render_drawer_layer(win, cx);
            let modal_layer = Root::render_modal_layer(win, cx);

            cx.new(|cx| Root::new(workspace_view.into(), win, cx))
        })
        .unwrap();
    });
}
