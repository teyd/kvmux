//! USB devices and their connect/disconnect events. Hardware sits behind [`UsbSource`].

use crate::config::UsbId;

mod fake;
mod kinds;
mod nusb_source;

pub use fake::FakeUsb;
pub use kinds::{DeviceKinds, InterfaceClass};
pub use nusb_source::NusbUsb;

/// A USB device as the UI and the daemon see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDevice {
    pub vendor_id: u16,
    pub product_id: u16,
    /// `None` when the device does not report one. Identical devices without serials
    /// cannot be told apart.
    pub serial: Option<String>,
    pub manufacturer: Option<String>,
    /// Name shown in the UI: the product string, or `VVVV:PPPP` when there is none.
    pub name: String,
    pub kinds: DeviceKinds,
}

impl UsbDevice {
    /// The config entry that selects this device as the trigger.
    pub fn to_id(&self) -> UsbId {
        UsbId {
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            serial: self.serial.clone(),
            label: self.name.clone(),
        }
    }

    /// Vendor and product id must match. A serial in the config must match too; a config
    /// without one matches every device with that vendor and product id.
    pub fn matches(&self, id: &UsbId) -> bool {
        self.vendor_id == id.vendor_id
            && self.product_id == id.product_id
            && id
                .serial
                .as_ref()
                .is_none_or(|serial| self.serial.as_ref() == Some(serial))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbEvent {
    Connected(UsbDevice),
    Disconnected(UsbDevice),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsbError {
    #[error("could not read the USB devices: {0}")]
    Enumerate(String),
    #[error("could not watch for USB devices: {0}")]
    Watch(String),
    /// The source has no more events and never will.
    #[error("the USB event stream ended")]
    Closed,
}

pub trait UsbSource {
    /// The devices attached right now.
    fn devices(&mut self) -> Result<Vec<UsbDevice>, UsbError>;

    /// Blocks until a device connects or disconnects. Events for devices that were
    /// already listed by [`UsbSource::devices`] are not repeated.
    fn wait_event(&mut self) -> Result<UsbEvent, UsbError>;

    /// Returns the next queued event without waiting, or `None` when idle.
    /// Allows application owners to service commands and stop watching promptly.
    fn poll_event(&mut self) -> Result<Option<UsbEvent>, UsbError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(serial: Option<&str>) -> UsbDevice {
        UsbDevice {
            vendor_id: 0x046d,
            product_id: 0xc52b,
            serial: serial.map(str::to_owned),
            manufacturer: Some("Logitech".into()),
            name: "USB Receiver".into(),
            kinds: DeviceKinds::default(),
        }
    }

    #[test]
    fn id_without_serial_matches_any_unit() {
        let id = device(None).to_id();
        assert!(device(Some("A")).matches(&id));
        assert!(device(None).matches(&id));
    }

    #[test]
    fn id_with_serial_matches_only_that_unit() {
        let id = device(Some("A")).to_id();
        assert!(device(Some("A")).matches(&id));
        assert!(!device(Some("B")).matches(&id));
        assert!(!device(None).matches(&id));
    }

    #[test]
    fn other_product_does_not_match() {
        let id = device(None).to_id();
        let mut other = device(None);
        other.product_id = 0x1234;
        assert!(!other.matches(&id));
    }
}
