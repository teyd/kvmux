use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    scroll::ScrollableElement as _,
    searchable_list::SearchableListItem,
    select::{Select, SelectEvent, SelectState},
};
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, TestSupportExt as _, Window, actions, div,
};
use kvmux_core::{
    Config, DdcDisplays, DisplaySource, Input, MonitorInfo, MonitorRule, NusbUsb, UsbDevice, UsbId,
    UsbSource,
};

use crate::runtime::{Command, Runtime, Update, config_path};

actions!(settings, [Save, Refresh]);

#[derive(Clone)]
struct Choice<T: Clone + PartialEq> {
    value: T,
    label: SharedString,
}

impl<T: Clone + PartialEq> SearchableListItem for Choice<T> {
    type Value = T;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &T {
        &self.value
    }
}

type UsbSelect = SelectState<Vec<Choice<Option<UsbId>>>>;
type InputSelect = SelectState<Vec<Choice<Option<Input>>>>;

struct DisplaySettings {
    info: MonitorInfo,
    present: bool,
    ambiguous: bool,
    enabled: bool,
    connect: Entity<InputSelect>,
    disconnect: Entity<InputSelect>,
}

/// Owns the editable form and the runtime; dropping it disconnects the worker.
pub struct SettingsView {
    runtime: Option<Runtime>,
    config: Config,
    devices: Vec<UsbDevice>,
    trigger: Entity<UsbSelect>,
    displays: Vec<DisplaySettings>,
    loaded: bool,
    loading: bool,
    saving: bool,
    load_error: Option<String>,
    usb_error: Option<String>,
    status: String,
    focus: FocusHandle,
    _updates: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub fn new(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        match config_path() {
            Ok(path) => Self::with_hardware(
                path,
                || {
                    let usb = NusbUsb::new()
                        .map(|usb| Box::new(usb) as Box<dyn UsbSource>)
                        .map_err(|error| format!("USB access failed: {error}. Check device permissions and restart kvmux."));
                    (usb, Box::new(DdcDisplays::new()) as Box<dyn DisplaySource>)
                },
                window,
                cx,
            ),
            Err(error) => {
                let mut view = Self::empty(window, cx);
                view.load_error = Some(error);
                view.loading = false;
                view
            }
        }
    }

    /// Injects the existing hardware traits for an end-to-end settings workflow.
    pub fn with_hardware(
        path: PathBuf,
        hardware: impl FnOnce() -> (Result<Box<dyn UsbSource>, String>, Box<dyn DisplaySource>)
        + Send
        + 'static,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let mut view = Self::empty(window, cx);
        view.runtime = Some(Runtime::start(path, hardware));
        view
    }

    fn empty(window: &mut Window, cx: &mut Context<'_, Self>) -> Self {
        let trigger = cx.new(|cx| {
            SelectState::new(
                vec![Choice {
                    value: None,
                    label: "No trigger".into(),
                }],
                None,
                window,
                cx,
            )
        });
        let focus = cx.focus_handle();
        let subscription = cx.subscribe(&trigger, |view, _, _: &SelectEvent<_>, cx| {
            view.mark_changed(cx);
        });
        focus.focus(window, cx);
        let updates = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update_in(cx, |view, window, cx| view.receive_updates(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            runtime: None,
            config: Config::default(),
            devices: vec![],
            trigger,
            displays: vec![],
            loaded: false,
            loading: true,
            saving: false,
            load_error: None,
            usb_error: None,
            status: "Loading configuration and discovering devices…".into(),
            focus,
            _updates: updates,
            _subscriptions: vec![subscription],
        }
    }

    fn receive_updates(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let Some(runtime) = &self.runtime else {
            return;
        };
        let mut messages = Vec::new();
        let disconnected = loop {
            match runtime.updates.try_recv() {
                Ok(message) => messages.push(message),
                Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
            }
        };
        if messages.is_empty() && !disconnected {
            return;
        }
        for message in messages {
            match message {
                Update::Loaded(result) => {
                    match result {
                        Ok(config) => {
                            self.config = config;
                            self.loaded = true;
                        }
                        Err(error) => self.load_error = Some(error),
                    }
                    self.sync_trigger(window, cx);
                }
                Update::Discovery { devices, monitors } => {
                    match devices {
                        Ok(devices) => {
                            self.devices = devices;
                            self.usb_error = None;
                        }
                        Err(error) => self.usb_error = Some(error),
                    }
                    self.sync_trigger(window, cx);
                    self.sync_displays(monitors, window, cx);
                    self.loading = false;
                    self.status = if self.usb_error.is_some() {
                        "USB watching is unavailable. Resolve the USB access error and restart."
                            .into()
                    } else {
                        "Watching USB devices. Save to apply your choices.".into()
                    };
                }
                Update::Devices(devices) => {
                    self.devices = devices;
                    self.sync_trigger(window, cx);
                }
                Update::Saved(result) => {
                    self.saving = false;
                    match result {
                        Ok(config) => {
                            self.config = config;
                            self.status = if self.usb_error.is_some() {
                                "Saved. USB watching is unavailable; resolve the access error and restart.".into()
                            } else if self.config.trigger.is_none() {
                                "Saved. Switching disabled.".into()
                            } else {
                                "Saved. Watching with the new configuration.".into()
                            };
                        }
                        Err(error) => self.status = error,
                    }
                }
                Update::Status(status) => self.status = status,
                Update::WatchFailed(error) => {
                    self.status = error.clone();
                    self.usb_error = Some(error);
                }
            }
        }
        if disconnected {
            self.runtime = None;
            self.loaded = false;
            self.loading = false;
            self.saving = false;
            self.status =
                "The hardware worker stopped. Restart kvmux to resume discovery and switching."
                    .into();
        }
        cx.notify();
    }

    fn sync_trigger(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let mut selected = self
            .trigger
            .read(cx)
            .selected_value()
            .cloned()
            .unwrap_or_else(|| self.config.trigger.clone());
        let mut choices = vec![Choice {
            value: None,
            label: "No trigger (disable switching)".into(),
        }];
        for device in &self.devices {
            let id = device.to_id();
            if choices.iter().any(|choice| {
                choice
                    .value
                    .as_ref()
                    .is_some_and(|known| same_usb(known, &id))
            }) {
                continue;
            }
            choices.push(Choice {
                value: Some(id),
                label: format!(
                    "{} · {:04x}:{:04x}{}",
                    device.name,
                    device.vendor_id,
                    device.product_id,
                    device
                        .serial
                        .as_ref()
                        .map(|serial| format!(" · {serial}"))
                        .unwrap_or_default()
                )
                .into(),
            });
        }
        if let Some(id) = &selected {
            if let Some(choice) = choices.iter().find(|choice| {
                choice
                    .value
                    .as_ref()
                    .is_some_and(|known| same_usb(known, id))
            }) {
                selected = choice.value.clone();
            } else {
                choices.push(Choice {
                    value: selected.clone(),
                    label: format!("{} · disconnected", id.label).into(),
                });
            }
        }
        self.trigger.update(cx, |state, cx| {
            state.set_items(choices, window, cx);
            state.set_selected_value(&selected, window, cx);
        });
    }

    fn sync_displays(
        &mut self,
        mut monitors: Vec<MonitorInfo>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let present_ids: Vec<_> = monitors.iter().map(|monitor| monitor.id.clone()).collect();
        for row in &mut self.displays {
            row.present = false;
            row.ambiguous = false;
        }
        for rule in &self.config.monitors {
            if !monitors.iter().any(|monitor| monitor.id == rule.id) {
                monitors.push(MonitorInfo {
                    id: rule.id.clone(),
                    label: rule.label.clone(),
                    current_input: None,
                    supported_inputs: vec![],
                });
            }
        }
        for info in monitors {
            // Config-only rows have no live EDID result; preserve them, don't silently discard rules.
            let present = present_ids.contains(&info.id);
            let ambiguous = present_ids.iter().filter(|id| **id == info.id).count() > 1;
            if let Some(row) = self.displays.iter_mut().find(|row| row.info.id == info.id) {
                row.info = info;
                row.present = present;
                row.ambiguous = ambiguous;
                continue;
            }
            let rule = self.config.monitors.iter().find(|rule| rule.id == info.id);
            let connect = rule
                .map(|rule| rule.on_connect)
                .or(info.current_input)
                .unwrap_or(Input::Hdmi1);
            let disconnect = rule.and_then(|rule| rule.on_disconnect);
            let mut inputs = if info.supported_inputs.is_empty() {
                Input::NAMED.to_vec()
            } else {
                info.supported_inputs.clone()
            };
            for input in [Some(connect), disconnect].into_iter().flatten() {
                if !inputs.contains(&input) {
                    inputs.push(input);
                }
            }
            let choices: Vec<_> = inputs
                .into_iter()
                .map(|input| Choice {
                    value: Some(input),
                    label: input.label().into(),
                })
                .collect();
            let connect_state = cx.new(|cx| {
                let mut state = SelectState::new(choices.clone(), None, window, cx);
                state.set_selected_value(&Some(connect), window, cx);
                state
            });
            let disconnect_state = cx.new(|cx| {
                let mut choices = choices;
                choices.insert(
                    0,
                    Choice {
                        value: None,
                        label: "Leave unchanged".into(),
                    },
                );
                let mut state = SelectState::new(choices, None, window, cx);
                state.set_selected_value(&disconnect, window, cx);
                state
            });
            for state in [&connect_state, &disconnect_state] {
                self._subscriptions
                    .push(cx.subscribe(state, |view, _, _: &SelectEvent<_>, cx| {
                        view.mark_changed(cx);
                    }));
            }
            self.displays.push(DisplaySettings {
                info,
                present,
                ambiguous,
                enabled: rule.is_some(),
                connect: connect_state,
                disconnect: disconnect_state,
            });
        }
    }

    fn mark_changed(&mut self, cx: &mut Context<'_, Self>) {
        self.status =
            "Unsaved changes. Save to apply; switching still uses the saved choices.".into();
        cx.notify();
    }

    fn save(&mut self, _: &Save, _: &mut Window, cx: &mut Context<'_, Self>) {
        if !self.loaded || self.loading || self.saving || self.load_error.is_some() {
            return;
        }
        let mut config = self.config.clone();
        config.trigger = self.trigger.read(cx).selected_value().cloned().flatten();
        config.monitors = self
            .displays
            .iter()
            .filter(|row| row.enabled)
            .map(|row| MonitorRule {
                id: row.info.id.clone(),
                label: row.info.label.clone(),
                on_connect: row
                    .connect
                    .read(cx)
                    .selected_value()
                    .copied()
                    .flatten()
                    .unwrap_or(Input::Hdmi1),
                on_disconnect: row.disconnect.read(cx).selected_value().copied().flatten(),
            })
            .collect();
        if config.trigger.is_some() && config.monitors.is_empty() {
            self.status =
                "Choose at least one display, or select No trigger to disable switching.".into();
        } else if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.commands.send(Command::Save(config)).is_ok())
        {
            self.saving = true;
            self.status = "Saving configuration…".into();
        } else {
            self.status = "The hardware worker stopped. Restart kvmux before saving.".into();
        }
        cx.notify();
    }

    fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<'_, Self>) {
        if self.loading || self.saving {
            return;
        }
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.commands.send(Command::Refresh).is_ok())
        {
            self.loading = true;
            self.status = "Refreshing displays…".into();
        }
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let busy = self.loading || self.saving;
        let mut content = div().flex().flex_col().gap_4().p_4()
            .child(div().id("title").role(Role::Heading).test_support().aria_label("Settings").text_lg().child("Settings"))
            .child(div().text_color(cx.theme().muted_foreground).child("Switch display inputs when your USB device connects. Keep this window open; closing it quits kvmux."))
            .child(div().child("USB trigger"))
            .child(Select::new(&self.trigger).id("usb-trigger").accessibility_label("USB trigger").disabled(busy))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child("Devices without serial numbers match every unit with the same vendor and product IDs."));
        if self.devices.is_empty() && !self.loading {
            content = content.child(
                div().child("No USB devices connected. Connect the trigger device to select it."),
            );
        }
        for error in [&self.load_error, &self.usb_error].into_iter().flatten() {
            content = content.child(div().text_color(cx.theme().danger).child(error.clone()));
        }
        content = content.child(div().text_lg().child("Displays"));
        if self.displays.is_empty() {
            content = content.child(div().child(if self.loading { "Discovering DDC/CI displays…" } else { "No DDC/CI displays found. Enable DDC/CI in the display menu and check the cable. On Linux, load i2c-dev and grant your user access to /dev/i2c-*, then Refresh." }));
        }
        for row in &self.displays {
            let id = row.info.id.clone();
            let checkbox_id = SharedString::from(format!("display-{id}"));
            let label = format!(
                "{} · {}{}",
                row.info.label,
                id,
                if row.present {
                    ""
                } else {
                    " · unavailable or not responding"
                }
            );
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Checkbox::new(checkbox_id)
                            .label(label)
                            .checked(row.enabled)
                            .disabled(busy)
                            .on_click(cx.listener(move |view, checked, _, cx| {
                                if let Some(row) =
                                    view.displays.iter_mut().find(|row| row.info.id == id)
                                {
                                    row.enabled = *checked;
                                }
                                view.mark_changed(cx);
                            })),
                    )
                    .child(div().child("On connection"))
                    .child(
                        Select::new(&row.connect)
                            .id(SharedString::from(format!("connect-{}", row.info.id)))
                            .accessibility_label(format!("{} on connection", row.info.label))
                            .disabled(busy || !row.enabled || row.ambiguous),
                    )
                    .child(div().child("On disconnection"))
                    .child(
                        Select::new(&row.disconnect)
                            .id(SharedString::from(format!("disconnect-{}", row.info.id)))
                            .accessibility_label(format!("{} on disconnection", row.info.label))
                            .disabled(busy || !row.enabled || row.ambiguous),
                    ),
            );
            if row.ambiguous {
                content = content.child(div().text_color(cx.theme().danger).child("Multiple displays share this EDID. Switching is blocked to avoid controlling the wrong display. Disconnect the duplicate or use displays with unique EDID serials."));
            } else if row.present && row.info.current_input.is_none() {
                content = content.child(div().text_sm().text_color(cx.theme().muted_foreground).child("The display did not answer an input read. Enable DDC/CI, check device permissions, or wait a few seconds after switching and Refresh."));
            }
        }
        div()
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::refresh))
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .child(content)
                    .overflow_y_scrollbar(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("refresh")
                                    .label("Refresh")
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.refresh(&Refresh, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("save")
                                    .primary()
                                    .label("Save")
                                    .disabled(busy || !self.loaded || self.load_error.is_some())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.save(&Save, window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("status")
                            .role(Role::Status)
                            .test_support()
                            .aria_label(self.status.clone())
                            .text_sm()
                            .child(self.status.clone()),
                    ),
            )
    }
}

fn same_usb(a: &UsbId, b: &UsbId) -> bool {
    (a.vendor_id, a.product_id, &a.serial) == (b.vendor_id, b.product_id, &b.serial)
}
