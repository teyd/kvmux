use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, TestAppContext, px, size};
use kvmux::settings::SettingsView;

#[gpui_kit::test]
fn settings_window_renders_its_title(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = cx.open_window(size(px(520.), px(400.)), |window, cx| {
        let view = cx.new(|_| SettingsView::new());
        Root::new(view, window, cx)
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let title = window.find("title");
        assert_eq!(title.label(), Some("Settings"));
        assert!(title.visible());
    })
    .expect("window is open");
}
