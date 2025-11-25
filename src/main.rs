mod assets;
mod services;
mod state;
mod themes;
mod window;
mod workspace;

use assets::*;
use gpui::{App, AppContext as _, Application, KeyBinding, actions};
use gpui_component::{ActiveTheme as _, Root, theme};
use themes::*;
use window::*;
use workspace::*;

actions!(window, [Quit]);

fn main() {
    // Create app w/ assets
    let application = Application::new().with_assets(Assets);

    application.run(|cx: &mut App| {
        // Close app on macOS close icon click
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        // Setup window options and workspace
        let window_options = get_window_options(cx);
        cx.open_window(window_options, |win, cx| {
            gpui_component::init(cx);
            theme::init(cx);
            state::init(cx);
            change_color_mode(cx.theme().mode, win, cx);

            let workspace_view = Workspace::view(win, cx);
            cx.new(|cx| Root::new(workspace_view, win, cx))
        })
        .unwrap();

        // Close app w/ cmd-q
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        // Bring app to front
        cx.activate(true);
    });
}
