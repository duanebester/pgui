mod assets;
mod services;
mod themes;
mod window;
mod workspace;

use assets::Assets;
use themes::*;
use window::*;

use gpui::*;
use gpui_component::{ActiveTheme as _, Root, init, theme};
use workspace::Workspace;

fn main() {
    let application = Application::new().with_assets(Assets);

    application.run(|cx: &mut App| {
        let window_options = get_window_options(cx);
        cx.open_window(window_options, |win, cx| {
            init(cx);
            theme::init(cx);
            change_color_mode(cx.theme().mode, win, cx);

            let workspace_view = Workspace::view(win, cx);
            cx.new(|cx| Root::new(workspace_view.into(), win, cx))
        })
        .unwrap();
    });
}
