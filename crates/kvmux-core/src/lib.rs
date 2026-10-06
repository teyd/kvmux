//! kvmux core: everything that does not need a window.

pub mod config;
pub mod display;
pub mod input;

pub use config::{Config, ConfigError, MonitorRule, UsbId};
pub use display::{DisplayError, DisplaySource, FakeDisplays, MonitorInfo};
pub use input::{Input, ParseInputError};
