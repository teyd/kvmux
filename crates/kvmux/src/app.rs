use std::process::ExitCode;

use fastframe_instance::{Claim, Slot};
use fastframe_tray::Tray;
use fastframe_update::{Receipt, Release};
use futures_channel::mpsc::{UnboundedSender, unbounded};
use futures_lite::StreamExt as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, Global, KeyBinding, QuitMode, WindowBounds,
    WindowOptions, actions, px, size,
};

use crate::instance;
use crate::settings::SettingsView;
use crate::tray::{self, TrayAction};
use crate::update::{self, CheckOutcome, UpdateState};

/// Reverse-DNS identity, used by the platform for notifications, taskbar grouping and as the
/// name of the single-instance slot.
pub const APP_ID: &str = "io.github.teyd.kvmux";

actions!(kvmux, [Quit]);

struct SettingsWindow(AnyWindowHandle);

impl Global for SettingsWindow {}

pub struct Options {
    /// Open the settings window at start. A second launch passes the same request on to the
    /// running copy.
    pub show_settings: bool,
    /// Arguments the updater passes back when it relaunches kvmux after an update.
    pub relaunch_arguments: Vec<String>,
    /// Set when the updater relaunched this copy; acknowledged once the app is up.
    pub receipt: Option<Receipt>,
}

/// Messages for the app's foreground loop. Tray, instance and worker threads send them.
enum Command {
    OpenSettings,
    PollTray,
    Checked(Result<CheckOutcome, String>),
    Installed(Result<(), String>),
}

/// Runs the application until [`Quit`]. Closing the settings window does not quit:
/// kvmux keeps switching monitors in the background.
///
/// If another kvmux already runs, hands the request to it and returns.
pub fn run(options: Options) -> ExitCode {
    let (sender, receiver) = unbounded::<Command>();

    let request = if options.show_settings {
        instance::OPEN_SETTINGS
    } else {
        instance::PING
    };
    let instance_sender = sender.clone();
    let _running = match Slot::new(APP_ID).claim(request, move |request| {
        instance::handle(request, || {
            let _ = instance_sender.unbounded_send(Command::OpenSettings);
        })
    }) {
        Claim::First(guard) => guard,
        Claim::Running(_) | Claim::Declined => return ExitCode::SUCCESS,
        Claim::Unanswered => {
            eprintln!("kvmux: another copy is running but did not answer");
            return ExitCode::FAILURE;
        }
    };

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            cx.set_app_identity(APP_ID, "kvmux");
            cx.bind_keys([KeyBinding::new("secondary-q", Quit, None)]);
            cx.on_action(|_: &Quit, cx| cx.quit());

            let wake = sender.clone();
            let tray = tray::spawn(move || {
                let _ = wake.unbounded_send(Command::PollTray);
            });
            if tray.is_none() {
                eprintln!("kvmux: no system tray; run `kvmux` again to open the settings");
            }

            if options.show_settings || options.receipt.is_some() {
                open_settings(cx);
            }

            if let Some(receipt) = options.receipt {
                // Never acknowledge a replacement before its window has rendered. If opening
                // fails, leave the receipt unacknowledged so the helper can roll back.
                if let Some(SettingsWindow(handle)) = cx.try_global::<SettingsWindow>() {
                    let _ = handle.update(cx, |_, window, _| {
                        window.on_next_frame(move |_, _| {
                            std::thread::spawn(move || {
                                if let Err(error) = receipt.acknowledge() {
                                    eprintln!("kvmux: could not acknowledge the update: {error:#}");
                                }
                            });
                        });
                    });
                }
            }

            let mut controller = Controller {
                tray,
                sender,
                relaunch_arguments: options.relaunch_arguments,
                state: UpdateState::Idle,
            };
            cx.spawn(async move |cx| {
                let mut receiver = receiver;
                while let Some(command) = receiver.next().await {
                    cx.update(|cx| controller.handle(command, cx));
                }
            })
            .detach();
        });
    ExitCode::SUCCESS
}

struct Controller {
    tray: Option<Tray>,
    sender: UnboundedSender<Command>,
    relaunch_arguments: Vec<String>,
    state: UpdateState,
}

impl Controller {
    fn handle(&mut self, command: Command, cx: &mut App) {
        match command {
            Command::OpenSettings => open_settings(cx),
            Command::PollTray => {
                let actions: Vec<TrayAction> = self
                    .tray
                    .as_ref()
                    .map(|tray| tray.events().iter().filter_map(tray::action).collect())
                    .unwrap_or_default();
                for action in actions {
                    self.perform(action, cx);
                }
            }
            Command::Checked(result) => {
                self.set_state(UpdateState::after_check(result));
            }
            Command::Installed(Ok(())) => {
                // The helper is waiting for this process to exit before it replaces the binary.
                cx.quit();
            }
            Command::Installed(Err(message)) => self.set_state(UpdateState::Failed(message)),
        }
    }

    fn perform(&mut self, action: TrayAction, cx: &mut App) {
        match action {
            TrayAction::OpenSettings => open_settings(cx),
            TrayAction::Quit => cx.quit(),
            TrayAction::CheckForUpdates if !self.state.is_busy() => {
                self.set_state(UpdateState::Checking);
                let sender = self.sender.clone();
                std::thread::spawn(move || {
                    let _ = sender.unbounded_send(Command::Checked(update::check()));
                });
            }
            TrayAction::InstallUpdate => {
                let UpdateState::Available {
                    release,
                    blocked: None,
                } = &self.state
                else {
                    return;
                };
                let release: Release = release.clone();
                self.set_state(UpdateState::Installing {
                    version: release.version.clone(),
                });
                let sender = self.sender.clone();
                let arguments = self.relaunch_arguments.clone();
                std::thread::spawn(move || {
                    let _ = sender
                        .unbounded_send(Command::Installed(update::install(&release, arguments)));
                });
            }
            TrayAction::CheckForUpdates => {}
        }
    }

    fn set_state(&mut self, state: UpdateState) {
        self.state = state;
        let view = self.state.view();
        if let Some(tray) = &mut self.tray {
            tray.set_label(tray::CHECK_UPDATES, view.check_label);
            tray.set_enabled(tray::CHECK_UPDATES, view.check_enabled);
            match view.install_label {
                Some(label) => {
                    tray.set_label(tray::INSTALL_UPDATE, label);
                    tray.set_enabled(tray::INSTALL_UPDATE, view.install_enabled);
                    tray.set_visible(tray::INSTALL_UPDATE, true);
                }
                None => tray.set_visible(tray::INSTALL_UPDATE, false),
            }
            tray.set_tooltip(view.tooltip);
        }
    }
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
