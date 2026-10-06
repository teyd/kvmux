use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Global, KeyBinding, QuitMode, WindowBounds,
    WindowOptions, actions, px, size,
};

use crate::settings::SettingsView;

/// Reverse-DNS identity, used by the platform for notifications and taskbar grouping.
pub const APP_ID: &str = "io.github.teyd.kvmux";

actions!(kvmux, [Quit]);

struct SettingsWindow(AnyWindowHandle);

impl Global for SettingsWindow {}

/// Runs the application until [`Quit`]. Closing the settings window does not quit:
/// kvmux keeps switching monitors in the background.
pub fn run(show_settings: bool) {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity(APP_ID, "kvmux");
            cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
            cx.on_action(|_: &Quit, cx| cx.quit());
            if show_settings {
                open_settings(cx);
            }
        });
}

/// Opens the settings window, or brings the existing one to the front.
pub fn open_settings(cx: &mut App) {
    if let Some(SettingsWindow(handle)) = cx.try_global::<SettingsWindow>() {
        let handle = *handle;
        if cx.windows().contains(&handle)
            && handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        {
            cx.activate(true);
            return;
        }
    }

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(520.), px(400.)),
            cx,
        ))),
        window_min_size: Some(size(px(400.), px(300.))),
        ..WindowOptions::default()
    };
    match gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| SettingsView::new())) {
        Ok((handle, _)) => {
            cx.set_global(SettingsWindow(handle));
            cx.activate(true);
        }
        Err(error) => eprintln!("kvmux: could not open the settings window: {error:#}"),
    }
}
