//! Monitors and how to switch them. Hardware sits behind [`DisplaySource`].

use crate::input::Input;

mod fake;

pub use fake::FakeDisplays;

/// A monitor as the UI and the daemon see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    /// Stable id built from the EDID. See [`id::edid_id`].
    pub id: String,
    /// Name shown in the UI.
    pub label: String,
    /// `None` when the monitor does not answer a read.
    pub current_input: Option<Input>,
    /// Inputs the monitor says it supports. Empty means unknown, not "none".
    pub supported_inputs: Vec<Input>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DisplayError {
    #[error("no monitor with id {0:?}")]
    NotFound(String),
    #[error("the monitor did not accept the change: {0}")]
    Ddc(String),
}

/// Measured on real monitors (Gigabyte MO27Q28G, M27Q X, Linux i2c-dev): after an input
/// change a monitor can ignore DDC reads for roughly 3 to 6 seconds. So do not read the
/// input back to verify a switch, and treat `current_input: None` as "try again later".
/// A full `monitors()` call takes about 3 seconds for two monitors.
pub trait DisplaySource {
    /// Every monitor that answers DDC/CI. A monitor that fails a read is still listed,
    /// with `current_input` unset.
    fn monitors(&mut self) -> Vec<MonitorInfo>;

    fn set_input(&mut self, monitor_id: &str, input: Input) -> Result<(), DisplayError>;
}
