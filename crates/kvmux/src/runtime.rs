//! One hardware owner serializes discovery, persistence and switching off the UI thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use kvmux_core::{Config, DisplaySource, MonitorInfo, UsbDevice, UsbEvent, UsbSource};

pub(crate) enum Command {
    Refresh,
    Save(Config),
}

pub(crate) enum Update {
    Loaded(Result<Config, String>),
    Discovery {
        devices: Result<Vec<UsbDevice>, String>,
        monitors: Vec<MonitorInfo>,
    },
    Devices(Vec<UsbDevice>),
    Saved(Result<Config, String>),
    Status(String),
    WatchFailed(String),
}

pub(crate) struct Runtime {
    pub commands: Sender<Command>,
    pub updates: Receiver<Update>,
}

impl Runtime {
    pub fn start(
        path: PathBuf,
        hardware: impl FnOnce() -> (Result<Box<dyn UsbSource>, String>, Box<dyn DisplaySource>)
        + Send
        + 'static,
    ) -> Self {
        let (commands, requests) = mpsc::channel();
        let (responses, updates) = mpsc::channel();
        thread::spawn(move || {
            let loaded = Config::load(&path).map_err(|error| {
                format!(
                    "Couldn’t load {}: {error}. Fix the file and restart before saving.",
                    path.display()
                )
            });
            let mut config = loaded.clone().unwrap_or_default();
            let load_ok = loaded.is_ok();
            let _ = responses.send(Update::Loaded(loaded));
            let (mut usb, mut displays) = hardware();
            let mut devices = match &mut usb {
                Ok(source) => match source.devices() {
                    Ok(devices) => devices,
                    Err(error) => {
                        usb = Err(format!(
                            "Couldn’t discover USB devices: {error}. Check device permissions and restart kvmux."
                        ));
                        Vec::new()
                    }
                },
                Err(_) => Vec::new(),
            };
            discover(&mut usb, displays.as_mut(), &mut devices, &responses);
            loop {
                match requests.recv_timeout(Duration::from_millis(100)) {
                    Ok(Command::Refresh) => {
                        discover(&mut usb, displays.as_mut(), &mut devices, &responses);
                    }
                    Ok(Command::Save(next)) => {
                        // A malformed existing file must never be silently overwritten.
                        let saved = if load_ok {
                            next.save(&path).map(|()| next).map_err(|error| {
                                format!("Couldn’t save {}: {error}. Check directory permissions and try Save again.", path.display())
                            })
                        } else {
                            Err("Fix the configuration file and restart before saving.".into())
                        };
                        if let Ok(next) = &saved {
                            config = next.clone();
                        }
                        let _ = responses.send(Update::Saved(saved));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
                let event = match &mut usb {
                    Ok(usb) => usb.poll_event(),
                    Err(_) => continue,
                };
                match event {
                    Ok(Some(event)) => {
                        match &event {
                            UsbEvent::Connected(device) => devices.push(device.clone()),
                            UsbEvent::Disconnected(device) => {
                                devices.retain(|known| known != device)
                            }
                        }
                        let _ = responses.send(Update::Devices(devices.clone()));
                        if let Some(status) = switch_event(&config, &event, displays.as_mut()) {
                            let _ = responses.send(Update::Status(status));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let message =
                            format!("USB watching stopped: {error}. Restart kvmux to retry.");
                        let _ = responses.send(Update::WatchFailed(message.clone()));
                        usb = Err(message);
                    }
                }
            }
        });
        Self { commands, updates }
    }
}

fn discover(
    usb: &mut Result<Box<dyn UsbSource>, String>,
    displays: &mut dyn DisplaySource,
    known: &mut Vec<UsbDevice>,
    responses: &Sender<Update>,
) {
    // Do not re-enumerate an active USB source: doing so would consume its identity
    // snapshot and lose pending disconnect/connect events during a slow DDC refresh.
    let devices = match usb {
        Ok(_) => Ok(known.clone()),
        Err(error) => Err(error.clone()),
    };
    if let Ok(devices) = &devices {
        *known = devices.clone();
    }
    let _ = responses.send(Update::Discovery {
        devices,
        monitors: displays.monitors(),
    });
}

fn switch_event(
    config: &Config,
    event: &UsbEvent,
    displays: &mut dyn DisplaySource,
) -> Option<String> {
    let (device, connected) = match event {
        UsbEvent::Connected(device) => (device, true),
        UsbEvent::Disconnected(device) => (device, false),
    };
    if !config
        .trigger
        .as_ref()
        .is_some_and(|trigger| device.matches(trigger))
    {
        return None;
    }
    let mut errors = Vec::new();
    let mut switched = 0;
    for rule in &config.monitors {
        let input = if connected {
            Some(rule.on_connect)
        } else {
            rule.on_disconnect
        };
        if let Some(input) = input {
            match displays.set_input(&rule.id, input) {
                Ok(()) => switched += 1,
                Err(error) => errors.push(format!("{}: {error}", rule.label)),
            }
        }
    }
    if errors.is_empty() {
        Some(format!(
            "{} {}. Switched {switched} display(s).",
            device.name,
            if connected {
                "connected"
            } else {
                "disconnected"
            }
        ))
    } else {
        Some(format!(
            "Couldn’t switch {}. Check the display cable, DDC/CI setting and device permissions, then reconnect the trigger.",
            errors.join("; ")
        ))
    }
}

/// Uses native user configuration locations without introducing another dependency.
pub(crate) fn config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    let directory = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(target_os = "windows"))]
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    directory.map(|directory| directory.join("kvmux/config.toml"))
        .ok_or_else(|| "Couldn’t locate your configuration directory. Set HOME (Linux) or APPDATA (Windows) and restart.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvmux_core::{DeviceKinds, FakeDisplays, FakeUsb, Input, MonitorRule};

    fn device(serial: &str) -> UsbDevice {
        UsbDevice {
            vendor_id: 1,
            product_id: 2,
            serial: Some(serial.into()),
            manufacturer: None,
            name: "Keyboard".into(),
            kinds: DeviceKinds::default(),
        }
    }

    #[test]
    fn switches_multiple_displays_by_identity_and_optional_disconnect() {
        let mut displays = FakeDisplays::new(
            ["B", "A"]
                .map(|id| MonitorInfo {
                    id: id.into(),
                    label: id.into(),
                    current_input: None,
                    supported_inputs: vec![],
                })
                .into(),
        );
        let config = Config {
            trigger: Some(device("one").to_id()),
            monitors: vec![
                MonitorRule {
                    id: "A".into(),
                    label: "A".into(),
                    on_connect: Input::Hdmi1,
                    on_disconnect: Some(Input::DisplayPort1),
                },
                MonitorRule {
                    id: "B".into(),
                    label: "B".into(),
                    on_connect: Input::Hdmi2,
                    on_disconnect: None,
                },
            ],
            ..Config::default()
        };
        assert!(
            switch_event(
                &config,
                &UsbEvent::Connected(device("other")),
                &mut displays
            )
            .is_none()
        );
        let mut usb = FakeUsb::default();
        usb.queue(UsbEvent::Connected(device("one")));
        usb.queue(UsbEvent::Disconnected(device("one")));
        while let Some(event) = usb.poll_event().expect("events") {
            switch_event(&config, &event, &mut displays);
        }
        assert_eq!(
            displays.switched(),
            [
                ("A".into(), Input::Hdmi1),
                ("B".into(), Input::Hdmi2),
                ("A".into(), Input::DisplayPort1)
            ]
        );
    }

    #[test]
    fn failure_on_one_display_does_not_skip_the_next() {
        let mut displays = FakeDisplays::new(vec![MonitorInfo {
            id: "B".into(),
            label: "B".into(),
            current_input: None,
            supported_inputs: vec![],
        }]);
        let config = Config {
            trigger: Some(device("one").to_id()),
            monitors: ["missing", "B"]
                .map(|id| MonitorRule {
                    id: id.into(),
                    label: id.into(),
                    on_connect: Input::Hdmi1,
                    on_disconnect: None,
                })
                .into(),
            ..Config::default()
        };
        let status = switch_event(&config, &UsbEvent::Connected(device("one")), &mut displays)
            .expect("matched");
        assert!(status.contains("Couldn’t switch missing"));
        assert_eq!(displays.switched(), [("B".into(), Input::Hdmi1)]);
    }

    struct ScriptedUsb {
        fake: FakeUsb,
        events: Receiver<UsbEvent>,
        stopped: Sender<()>,
    }

    impl UsbSource for ScriptedUsb {
        fn devices(&mut self) -> Result<Vec<UsbDevice>, kvmux_core::UsbError> {
            self.fake.devices()
        }
        fn wait_event(&mut self) -> Result<UsbEvent, kvmux_core::UsbError> {
            self.fake.wait_event()
        }
        fn poll_event(&mut self) -> Result<Option<UsbEvent>, kvmux_core::UsbError> {
            if let Ok(event) = self.events.try_recv() {
                self.fake.queue(event);
            }
            self.fake.poll_event()
        }
    }

    impl Drop for ScriptedUsb {
        fn drop(&mut self) {
            let _ = self.stopped.send(());
        }
    }

    struct RecordingDisplays {
        fake: FakeDisplays,
        switched: Sender<Input>,
    }

    impl DisplaySource for RecordingDisplays {
        fn monitors(&mut self) -> Vec<MonitorInfo> {
            self.fake.monitors()
        }
        fn set_input(&mut self, id: &str, input: Input) -> Result<(), kvmux_core::DisplayError> {
            self.fake.set_input(id, input)?;
            let _ = self.switched.send(input);
            Ok(())
        }
    }

    #[test]
    fn running_worker_applies_only_successful_saves_and_releases_usb_on_close() {
        let directory = std::env::temp_dir().join(format!("kvmux-runtime-{}", std::process::id()));
        let path = directory.join("config.toml");
        let (events, input_events) = mpsc::channel();
        let (stopped, stop_signal) = mpsc::channel();
        let (switched, switches) = mpsc::channel();
        let runtime = Runtime::start(path.clone(), move || {
            (
                Ok(Box::new(ScriptedUsb {
                    fake: FakeUsb::new(vec![device("one")]),
                    events: input_events,
                    stopped,
                })),
                Box::new(RecordingDisplays {
                    fake: FakeDisplays::new(vec![MonitorInfo {
                        id: "A".into(),
                        label: "A".into(),
                        current_input: None,
                        supported_inputs: vec![],
                    }]),
                    switched,
                }),
            )
        });
        let mut config = Config {
            trigger: Some(device("one").to_id()),
            monitors: vec![MonitorRule {
                id: "A".into(),
                label: "A".into(),
                on_connect: Input::Hdmi1,
                on_disconnect: Some(Input::DisplayPort1),
            }],
            ..Config::default()
        };
        runtime
            .commands
            .send(Command::Save(config.clone()))
            .expect("send save");
        loop {
            if let Update::Saved(result) = runtime
                .updates
                .recv_timeout(Duration::from_secs(2))
                .expect("response")
            {
                assert_eq!(result.expect("saved"), config);
                break;
            }
        }
        assert_eq!(Config::load(&path).expect("persisted"), config);
        events
            .send(UsbEvent::Connected(device("one")))
            .expect("event");
        assert_eq!(
            switches
                .recv_timeout(Duration::from_secs(2))
                .expect("switch"),
            Input::Hdmi1
        );
        config.monitors[0].on_connect = Input::Hdmi2;
        runtime
            .commands
            .send(Command::Save(config.clone()))
            .expect("send save");
        loop {
            if let Update::Saved(result) = runtime
                .updates
                .recv_timeout(Duration::from_secs(2))
                .expect("response")
            {
                assert_eq!(result.expect("saved"), config);
                break;
            }
        }
        events
            .send(UsbEvent::Connected(device("one")))
            .expect("event");
        assert_eq!(
            switches
                .recv_timeout(Duration::from_secs(2))
                .expect("new switch"),
            Input::Hdmi2
        );

        std::fs::remove_file(&path).expect("remove fixture file");
        std::fs::create_dir(&path).expect("block persistence");
        config.monitors[0].on_connect = Input::Vga1;
        runtime
            .commands
            .send(Command::Save(config))
            .expect("send save");
        loop {
            if let Update::Saved(result) = runtime
                .updates
                .recv_timeout(Duration::from_secs(2))
                .expect("response")
            {
                assert!(result.is_err());
                break;
            }
        }
        events
            .send(UsbEvent::Connected(device("one")))
            .expect("event");
        assert_eq!(
            switches
                .recv_timeout(Duration::from_secs(2))
                .expect("previous switch"),
            Input::Hdmi2
        );
        events
            .send(UsbEvent::Disconnected(device("one")))
            .expect("event");
        assert_eq!(
            switches
                .recv_timeout(Duration::from_secs(2))
                .expect("disconnect switch"),
            Input::DisplayPort1
        );
        drop(runtime);
        stop_signal
            .recv_timeout(Duration::from_secs(2))
            .expect("watcher stopped without another event");
        std::fs::remove_dir_all(directory).expect("remove fixture directory");
    }
}
