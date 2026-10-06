use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AnyWindowHandle, AppContext as _, TestAppContext, px, size};
use kvmux::settings::SettingsView;
use kvmux_core::{
    Config, DeviceKinds, FakeDisplays, FakeUsb, Input, MonitorInfo, MonitorRule, UsbDevice,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(
            std::env::temp_dir()
                .join(format!(
                    "kvmux-ui-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ))
                .join("config.toml"),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(directory) = self.0.parent() {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
}

fn device() -> UsbDevice {
    UsbDevice {
        vendor_id: 0x046d,
        product_id: 0xc52b,
        serial: Some("receiver-1".into()),
        manufacturer: None,
        name: "USB Receiver".into(),
        kinds: DeviceKinds::default(),
    }
}

fn open(cx: &mut TestAppContext, path: PathBuf) -> AnyWindowHandle {
    cx.open_window(size(px(600.), px(850.)), |window, cx| {
        let view = cx.new(|cx| {
            SettingsView::with_hardware(
                path,
                || {
                    (
                        Ok(Box::new(FakeUsb::new(vec![device()]))),
                        Box::new(FakeDisplays::new(vec![MonitorInfo {
                            id: "DEL:123:serial".into(),
                            label: "Dell".into(),
                            current_input: Some(Input::Hdmi1),
                            supported_inputs: vec![Input::Hdmi1, Input::DisplayPort1],
                        }])),
                    )
                },
                window,
                cx,
            )
        });
        Root::new(view, window, cx)
    })
    .into()
}

fn wait_status(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
    // Hardware runs on a real OS thread; give it bounded real time, then drive the
    // UI's simulated timer. Never wait while borrowing the GPUI window.
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(10));
        cx.background_executor
            .advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        let ready = cx
            .update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window
                    .find("status")
                    .label()
                    .is_some_and(|label| label.contains(text))
            })
            .expect("open window");
        if ready {
            return;
        }
    }
    panic!("status did not contain {text:?}");
}

#[gpui_kit::test]
fn configures_saves_and_restores_through_real_controls(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    cx.update(kvmux::app::init);
    let handle = open(cx, fixture.0.clone());
    wait_status(cx, handle, "Watching USB");

    cx.update_window(handle, |_, window, cx| {
        assert_eq!(window.find("title").label(), Some("Settings"));
        window.click("connect-DEL:123:serial", cx);
        assert_eq!(
            window.find("connect-DEL:123:serial").value(),
            Some("HDMI 1")
        );
        window.click("usb-trigger", cx);
    })
    .expect("window");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("window");
    cx.run_until_parked();
    // A trigger without a display is invalid, and must not write the file.
    cx.update_window(handle, |_, window, cx| {
        window.click("save", cx);
    })
    .expect("window");
    assert!(!fixture.0.exists());
    cx.update_window(handle, |_, window, cx| {
        assert!(
            window
                .find("status")
                .label()
                .expect("status")
                .contains("Choose at least one display")
        );
        window.click("display-DEL:123:serial", cx);
        assert_eq!(window.find("display-DEL:123:serial").checked(), Some(true));
        window.click("connect-DEL:123:serial", cx);
    })
    .expect("window");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("down", cx);
        window.press("enter", cx);
    })
    .expect("window");
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.press("secondary-s", cx);
    })
    .expect("window");
    wait_status(cx, handle, "Saved.");
    let saved = Config::load(&fixture.0).expect("saved config");
    assert_eq!(saved.trigger, Some(device().to_id()));
    assert_eq!(saved.monitors.len(), 1);
    assert_eq!(saved.monitors[0].id, "DEL:123:serial");
    assert_eq!(saved.monitors[0].on_connect, Input::DisplayPort1);
    assert_eq!(saved.monitors[0].on_disconnect, None);

    let restored = open(cx, fixture.0.clone());
    wait_status(cx, restored, "Watching USB");
    cx.update_window(restored, |_, window, _| {
        assert_eq!(window.find("display-DEL:123:serial").checked(), Some(true));
        assert_eq!(
            window.find("connect-DEL:123:serial").value(),
            Some("DisplayPort 1")
        );
        assert!(
            window
                .find("usb-trigger")
                .value()
                .expect("trigger")
                .contains("receiver-1")
        );
        assert!(window.find("save").visible());
        assert!(window.find("status").bounds().top() >= window.find("save").bounds().bottom());
    })
    .expect("window");
}

#[gpui_kit::test]
fn saving_failure_keeps_the_form(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    cx.update(kvmux::app::init);
    let handle = open(cx, fixture.0.clone());
    wait_status(cx, handle, "Watching USB");
    // Make the target a directory after loading: atomic replacement must fail.
    std::fs::create_dir_all(&fixture.0).expect("directory at destination");
    cx.update_window(handle, |_, window, cx| {
        window.click("save", cx);
    })
    .expect("window");
    wait_status(cx, handle, "Couldn’t save");
    cx.update_window(handle, |_, window, cx| {
        window.press("tab", cx);
        window.render_frame(cx);
        assert!(window.find("usb-trigger").visible());
        assert!(
            window
                .find("status")
                .label()
                .expect("status")
                .contains("permissions")
        );
        assert_ne!(window.find("save").disabled(), Some(true));
    })
    .expect("window");
}

#[gpui_kit::test]
fn form_and_footer_remain_usable_at_minimum_size_and_zoom(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    cx.update(kvmux::app::init);
    let handle = open(cx, fixture.0.clone());
    wait_status(cx, handle, "Watching USB");
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        for font_size in [16., 20.] {
            cx.update(|cx| {
                Theme::change(mode, None, cx);
                Theme::update(cx, |theme| theme.font_size = px(font_size));
            });
            cx.update_window(handle, |_, window, cx| {
                window.resize(size(px(420.), px(400.)));
                window.render_frame(cx);
                let save = window.find("save");
                let refresh = window.find("refresh");
                assert!(save.visible());
                assert!(refresh.visible());
                assert!(save.bounds().bottom() <= window.viewport_size().height);
                assert_eq!(save.bounds().top(), refresh.bounds().top());
                assert_eq!(
                    window.find("title").bounds().left(),
                    refresh.bounds().left()
                );
                assert!(window.find("usb-trigger").visible());
            })
            .expect("window");
        }
    }
}

#[gpui_kit::test]
fn missing_devices_and_raw_inputs_survive_refresh(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    let mut missing = device().to_id();
    missing.serial = Some("disconnected-receiver".into());
    let config = Config {
        trigger: Some(missing),
        monitors: vec![MonitorRule {
            id: "LEN:456:missing".into(),
            label: "Disconnected display".into(),
            on_connect: Input::Raw(0x1b),
            on_disconnect: Some(Input::Hdmi2),
        }],
        ..Config::default()
    };
    config.save(&fixture.0).expect("saved fixture");
    cx.update(kvmux::app::init);
    let handle = open(cx, fixture.0.clone());
    wait_status(cx, handle, "Watching USB");
    cx.update_window(handle, |_, window, cx| {
        assert!(
            window
                .find("usb-trigger")
                .value()
                .expect("trigger")
                .contains("disconnected")
        );
        assert_eq!(window.find("display-LEN:456:missing").checked(), Some(true));
        window.click("refresh", cx);
    })
    .expect("window");
    wait_status(cx, handle, "Watching USB");
    cx.update_window(handle, |_, window, cx| {
        window.click("save", cx);
    })
    .expect("window");
    wait_status(cx, handle, "Saved.");
    assert_eq!(Config::load(&fixture.0).expect("saved config"), config);
}

#[gpui_kit::test]
fn malformed_config_is_not_overwritten(cx: &mut TestAppContext) {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.0.parent().expect("parent")).expect("directory");
    let text = "version = 99\n";
    std::fs::write(&fixture.0, text).expect("fixture");
    cx.update(kvmux::app::init);
    let handle = open(cx, fixture.0.clone());
    wait_status(cx, handle, "Watching USB");
    cx.update_window(handle, |_, window, cx| {
        window.click("save", cx);
        window.press("secondary-s", cx);
    })
    .expect("window");
    assert_eq!(
        std::fs::read_to_string(&fixture.0).expect("unchanged"),
        text
    );
}
