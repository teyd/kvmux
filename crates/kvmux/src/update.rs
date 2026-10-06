//! Self-update through GitHub releases, using fastframe-update.
//!
//! Every release is authorized by an Ed25519 signature over `checksums.txt`, checked against
//! the key embedded below before anything is parsed or written to disk.

use fastframe_update::{Release, ReqwestTransport, UpdateConfig, Updater};

/// `publisher_key` must always be set: without it fastframe-update falls back to a checksum
/// from the same release page, which proves nothing about who published it.
///
/// The key in `assets/update-public-key.hex` is a placeholder whose private half was destroyed.
/// No release can be signed for it, so updates fail closed. Replace it with the real
/// publisher key before the first release.
pub const UPDATES: UpdateConfig = UpdateConfig {
    publisher_key: Some(include_str!("../assets/update-public-key.hex")),
    ..UpdateConfig::new("teyd/kvmux", "kvmux", "kvmux", env!("CARGO_PKG_VERSION"))
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate,
    Available {
        release: Release,
        /// Why this copy cannot replace itself (a package manager owns it, or it is not a
        /// portable install). `None` means it can.
        blocked: Option<String>,
    },
}

fn updater() -> Result<Updater, String> {
    let transport = ReqwestTransport::new(reqwest::blocking::Client::builder())
        .map_err(|error| format!("could not set up the update client: {error}"))?;
    Ok(Updater::new(UPDATES, transport))
}

/// Blocking: call it from a worker thread.
pub fn check() -> Result<CheckOutcome, String> {
    let updater = updater()?;
    match updater.check().map_err(|error| format!("{error:#}"))? {
        None => Ok(CheckOutcome::UpToDate),
        Some(release) => {
            let blocked = updater
                .installation()
                .err()
                .map(|reason| reason.to_string());
            Ok(CheckOutcome::Available { release, blocked })
        }
    }
}

/// Downloads and verifies `release`, then hands over to the helper. On success the caller must
/// quit right away: the helper waits for this process to exit before it replaces the binary.
/// Blocking: call it from a worker thread.
pub fn install(release: &Release, relaunch_arguments: Vec<String>) -> Result<(), String> {
    let updater = updater()?;
    let prepared = updater
        .download(release, |_, _| {})
        .map_err(|error| format!("{error:#}"))?;
    updater
        .handoff(prepared, relaunch_arguments)
        .map_err(|error| format!("{error:#}"))
}

/// Where the update flow is, as the tray shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        release: Release,
        blocked: Option<String>,
    },
    Installing {
        version: String,
    },
    Failed(String),
}

/// What the tray menu should show for an [`UpdateState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayView {
    pub check_label: String,
    pub check_enabled: bool,
    /// `None` hides the install entry.
    pub install_label: Option<String>,
    pub install_enabled: bool,
    pub tooltip: String,
}

impl UpdateState {
    pub fn after_check(result: Result<CheckOutcome, String>) -> Self {
        match result {
            Ok(CheckOutcome::UpToDate) => Self::UpToDate,
            Ok(CheckOutcome::Available { release, blocked }) => {
                Self::Available { release, blocked }
            }
            Err(message) => Self::Failed(message),
        }
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Installing { .. })
    }

    pub fn view(&self) -> TrayView {
        let mut view = TrayView {
            check_label: "Check for updates".into(),
            check_enabled: true,
            install_label: None,
            install_enabled: true,
            tooltip: format!("kvmux {}", env!("CARGO_PKG_VERSION")),
        };
        match self {
            Self::Idle => {}
            Self::Checking => {
                view.check_label = "Checking for updates".into();
                view.check_enabled = false;
            }
            Self::UpToDate => view.tooltip.push_str("\nUp to date"),
            Self::Available {
                release,
                blocked: None,
            } => {
                view.install_label = Some(format!("Update to {}", release.version));
                view.tooltip
                    .push_str(&format!("\nUpdate {} available", release.version));
            }
            Self::Available {
                release,
                blocked: Some(reason),
            } => {
                view.check_label = format!("Update {} available", release.version);
                view.tooltip.push_str(&format!(
                    "\nUpdate {} available, but this copy cannot update itself: {reason}",
                    release.version
                ));
            }
            Self::Installing { version } => {
                view.check_enabled = false;
                view.install_label = Some(format!("Updating to {version}"));
                view.install_enabled = false;
            }
            Self::Failed(message) => view
                .tooltip
                .push_str(&format!("\nUpdate failed: {message}")),
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_publisher_key_is_always_embedded() {
        let key = UPDATES.publisher_key.expect("updates must be signed");
        let key = key.trim();
        assert_eq!(key.len(), 64, "an Ed25519 public key is 32 bytes of hex");
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(
            key.bytes().any(|byte| byte != b'0'),
            "an all-zero key is a weak point"
        );
        assert!(UPDATES.additional_publisher_keys.is_empty());
    }

    #[test]
    fn updates_come_from_this_repository_and_never_from_prereleases() {
        assert_eq!(UPDATES.repository, "teyd/kvmux");
        assert_eq!(UPDATES.slug, "kvmux");
        assert!(matches!(
            UPDATES.prereleases,
            fastframe_update::Prereleases::Never
        ));
        assert_eq!(UPDATES.current_version, env!("CARGO_PKG_VERSION"));
    }

    fn release(version: &str) -> Release {
        Release {
            version: version.into(),
            url: format!("https://github.com/teyd/kvmux/releases/tag/v{version}"),
        }
    }

    #[test]
    fn idle_offers_a_check_and_hides_the_install_entry() {
        let view = UpdateState::Idle.view();
        assert_eq!(view.check_label, "Check for updates");
        assert!(view.check_enabled);
        assert_eq!(view.install_label, None);
    }

    #[test]
    fn checking_disables_the_check_entry() {
        let view = UpdateState::Checking.view();
        assert!(!view.check_enabled);
        assert!(UpdateState::Checking.is_busy());
    }

    #[test]
    fn an_installable_update_shows_the_install_entry() {
        let state = UpdateState::after_check(Ok(CheckOutcome::Available {
            release: release("0.2.0"),
            blocked: None,
        }));
        let view = state.view();
        assert_eq!(view.install_label.as_deref(), Some("Update to 0.2.0"));
        assert!(view.install_enabled);
        assert!(view.tooltip.contains("Update 0.2.0 available"));
    }

    #[test]
    fn a_blocked_update_explains_why_and_offers_no_install() {
        let state = UpdateState::after_check(Ok(CheckOutcome::Available {
            release: release("0.2.0"),
            blocked: Some("installed by apt".into()),
        }));
        let view = state.view();
        assert_eq!(view.install_label, None);
        assert_eq!(view.check_label, "Update 0.2.0 available");
        assert!(view.tooltip.contains("installed by apt"));
    }

    #[test]
    fn installing_locks_both_entries() {
        let state = UpdateState::Installing {
            version: "0.2.0".into(),
        };
        let view = state.view();
        assert!(!view.check_enabled && !view.install_enabled);
        assert_eq!(view.install_label.as_deref(), Some("Updating to 0.2.0"));
        assert!(state.is_busy());
    }

    #[test]
    fn failures_keep_the_app_usable_and_say_what_happened() {
        let state = UpdateState::after_check(Err("offline".into()));
        assert!(!state.is_busy());
        let view = state.view();
        assert!(view.check_enabled);
        assert!(view.tooltip.contains("Update failed: offline"));
    }

    #[test]
    fn up_to_date_is_not_busy() {
        let state = UpdateState::after_check(Ok(CheckOutcome::UpToDate));
        assert_eq!(state, UpdateState::UpToDate);
        assert!(!state.is_busy());
    }
}
