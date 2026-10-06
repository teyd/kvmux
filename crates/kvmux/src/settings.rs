use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    Context, InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, TestSupportExt as _, Window, div,
};

/// The settings window content.
pub struct SettingsView;

impl SettingsView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_2()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("title")
                    .role(Role::Heading)
                    .test_support()
                    .aria_label("Settings")
                    .text_lg()
                    .child("Settings"),
            )
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child("kvmux runs in the background."),
            )
    }
}
