use std::collections::VecDeque;

use super::{UsbDevice, UsbError, UsbEvent, UsbSource};

/// Scripted USB devices and events for tests. `wait_event` returns the queued events in order,
/// then [`UsbError::Closed`].
#[derive(Debug, Default)]
pub struct FakeUsb {
    devices: Vec<UsbDevice>,
    events: VecDeque<UsbEvent>,
}

impl FakeUsb {
    pub fn new(devices: Vec<UsbDevice>) -> Self {
        Self {
            devices,
            events: VecDeque::new(),
        }
    }

    pub fn queue(&mut self, event: UsbEvent) {
        self.events.push_back(event);
    }
}

impl UsbSource for FakeUsb {
    fn devices(&mut self) -> Result<Vec<UsbDevice>, UsbError> {
        Ok(self.devices.clone())
    }

    fn wait_event(&mut self) -> Result<UsbEvent, UsbError> {
        let event = self.events.pop_front().ok_or(UsbError::Closed)?;
        match &event {
            UsbEvent::Connected(device) => self.devices.push(device.clone()),
            UsbEvent::Disconnected(device) => self.devices.retain(|known| known != device),
        }
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::DeviceKinds;

    fn device(product_id: u16) -> UsbDevice {
        UsbDevice {
            vendor_id: 1,
            product_id,
            serial: None,
            manufacturer: None,
            name: format!("device {product_id}"),
            kinds: DeviceKinds::default(),
        }
    }

    #[test]
    fn events_update_the_device_list_in_order() {
        let mut usb = FakeUsb::new(vec![device(1)]);
        usb.queue(UsbEvent::Connected(device(2)));
        usb.queue(UsbEvent::Disconnected(device(1)));

        assert_eq!(usb.wait_event(), Ok(UsbEvent::Connected(device(2))));
        assert_eq!(usb.devices().expect("devices"), [device(1), device(2)]);
        assert_eq!(usb.wait_event(), Ok(UsbEvent::Disconnected(device(1))));
        assert_eq!(usb.devices().expect("devices"), [device(2)]);
    }

    #[test]
    fn runs_dry_with_closed() {
        let mut usb = FakeUsb::default();
        assert_eq!(usb.wait_event(), Err(UsbError::Closed));
    }
}
