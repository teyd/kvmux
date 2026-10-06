//! The tray icon and its menu. The tray runs on its own thread and reports clicks as events.

use fastframe_tray::{Config, Event, MenuItem, Tray};

pub const SETTINGS: &str = "settings";
pub const CHECK_UPDATES: &str = "check_updates";
pub const INSTALL_UPDATE: &str = "install_update";
pub const QUIT: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    OpenSettings,
    CheckForUpdates,
    InstallUpdate,
    Quit,
}

/// What a tray event asks the app to do.
pub fn action(event: &Event) -> Option<TrayAction> {
    match event {
        Event::Toggle | Event::Show => Some(TrayAction::OpenSettings),
        Event::Menu(SETTINGS) => Some(TrayAction::OpenSettings),
        Event::Menu(CHECK_UPDATES) => Some(TrayAction::CheckForUpdates),
        Event::Menu(INSTALL_UPDATE) => Some(TrayAction::InstallUpdate),
        Event::Menu(QUIT) => Some(TrayAction::Quit),
        Event::Menu(_) => None,
    }
}

fn menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action(SETTINGS, "Settings"),
        MenuItem::action(CHECK_UPDATES, "Check for updates"),
        MenuItem::Action {
            id: INSTALL_UPDATE,
            label: "Update".into(),
            visible: false,
            enabled: true,
        },
        MenuItem::Separator,
        MenuItem::action(QUIT, "Quit"),
    ]
}

fn config() -> Config {
    Config {
        id: "kvmux",
        title: "kvmux".into(),
        icon,
        template_icon: None,
        themed_icon: false,
        menu_on_click: false,
        menu: menu(),
    }
}

/// `None` when the tray cannot be made at all. On Linux a missing tray host (GNOME without the
/// AppIndicator extension) is not an error: the item appears once a host shows up.
///
/// macOS is not wired up: status items need AppKit's main thread, which GPUI owns there.
pub fn spawn(wake: impl Fn() + Send + Sync + 'static) -> Option<Tray> {
    if cfg!(target_os = "macos") {
        return None;
    }
    Tray::spawn(config(), wake)
}

/// A rounded square with a smaller square inside, as RGBA. This is raster artwork, not UI
/// styling, so the colors are fixed.
fn icon(size: usize) -> Vec<u8> {
    const OUTER: [u8; 4] = [0x3b, 0x82, 0xf6, 0xff];
    const INNER: [u8; 4] = [0xff, 0xff, 0xff, 0xff];
    let size_f = size as f32;
    let radius = size_f * 0.22;
    let mut pixels = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let pixel = if inside_rounded_square(fx, fy, size_f, radius) {
                let margin = size_f * 0.3;
                if (margin..size_f - margin).contains(&fx)
                    && (margin..size_f - margin).contains(&fy)
                {
                    INNER
                } else {
                    OUTER
                }
            } else {
                [0, 0, 0, 0]
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    pixels
}

fn inside_rounded_square(x: f32, y: f32, size: f32, radius: f32) -> bool {
    let dx = (x - size / 2.0).abs() - (size / 2.0 - radius);
    let dy = (y - size / 2.0).abs() - (size / 2.0 - radius);
    dx.max(0.0).hypot(dy.max(0.0)) <= radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_map_to_actions() {
        assert_eq!(action(&Event::Toggle), Some(TrayAction::OpenSettings));
        assert_eq!(action(&Event::Show), Some(TrayAction::OpenSettings));
        assert_eq!(
            action(&Event::Menu(SETTINGS)),
            Some(TrayAction::OpenSettings)
        );
        assert_eq!(
            action(&Event::Menu(CHECK_UPDATES)),
            Some(TrayAction::CheckForUpdates)
        );
        assert_eq!(
            action(&Event::Menu(INSTALL_UPDATE)),
            Some(TrayAction::InstallUpdate)
        );
        assert_eq!(action(&Event::Menu(QUIT)), Some(TrayAction::Quit));
        assert_eq!(action(&Event::Menu("unknown")), None);
    }

    #[test]
    fn every_menu_entry_has_an_action_and_ids_are_unique() {
        let ids: Vec<&str> = menu()
            .iter()
            .filter_map(|item| match item {
                MenuItem::Action { id, .. } => Some(*id),
                MenuItem::Separator => None,
            })
            .collect();
        for id in &ids {
            assert!(action(&Event::Menu(id)).is_some(), "{id} does nothing");
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }

    #[test]
    fn the_update_entry_starts_hidden() {
        let hidden = menu().iter().any(|item| {
            matches!(item, MenuItem::Action { id, visible: false, .. } if *id == INSTALL_UPDATE)
        });
        assert!(hidden);
    }

    #[test]
    fn the_icon_is_rgba_with_transparent_corners_and_a_solid_center() {
        for size in [16, 32, 64] {
            let pixels = icon(size);
            assert_eq!(pixels.len(), size * size * 4);
            assert_eq!(pixels[3], 0, "corner is transparent");
            let center = ((size / 2) * size + size / 2) * 4;
            assert_eq!(pixels[center + 3], 0xff, "center is opaque");
        }
    }
}
