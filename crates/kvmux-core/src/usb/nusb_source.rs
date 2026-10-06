use std::collections::HashMap;

use futures_lite::StreamExt as _;
use futures_lite::future::block_on;
use nusb::hotplug::{HotplugEvent, HotplugWatch};
use nusb::{DeviceId, DeviceInfo, MaybeFuture as _};

use super::{DeviceKinds, InterfaceClass, UsbDevice, UsbError, UsbEvent, UsbSource};

/// USB devices through `nusb`: sysfs and netlink on Linux, SetupAPI on Windows, IOKit on macOS.
pub struct NusbUsb {
    watch: HotplugWatch,
    /// A disconnect event only carries an opaque id, so remember what each id was.
    known: HashMap<DeviceId, UsbDevice>,
}

impl NusbUsb {
    /// Starts watching before the first listing, so a device plugged in between the two
    /// is reported as an event instead of being missed.
    pub fn new() -> Result<Self, UsbError> {
        let watch = nusb::watch_devices().map_err(|error| UsbError::Watch(error.to_string()))?;
        let mut usb = Self {
            watch,
            known: HashMap::new(),
        };
        usb.devices()?;
        Ok(usb)
    }
}

impl UsbSource for NusbUsb {
    fn devices(&mut self) -> Result<Vec<UsbDevice>, UsbError> {
        let infos = nusb::list_devices()
            .wait()
            .map_err(|error| UsbError::Enumerate(error.to_string()))?;
        self.known = infos.map(|info| (info.id(), describe(&info))).collect();
        let mut devices: Vec<UsbDevice> = self.known.values().cloned().collect();
        devices.sort_by(|a, b| {
            (a.vendor_id, a.product_id, &a.serial).cmp(&(b.vendor_id, b.product_id, &b.serial))
        });
        Ok(devices)
    }

    fn wait_event(&mut self) -> Result<UsbEvent, UsbError> {
        loop {
            match block_on(self.watch.next()).ok_or(UsbError::Closed)? {
                HotplugEvent::Connected(info) => {
                    if self.known.contains_key(&info.id()) {
                        continue;
                    }
                    let device = describe(&info);
                    self.known.insert(info.id(), device.clone());
                    return Ok(UsbEvent::Connected(device));
                }
                HotplugEvent::Disconnected(id) => {
                    if let Some(device) = self.known.remove(&id) {
                        return Ok(UsbEvent::Disconnected(device));
                    }
                }
            }
        }
    }
}

fn describe(info: &DeviceInfo) -> UsbDevice {
    let mut interfaces: Vec<InterfaceClass> = info
        .interfaces()
        .map(|interface| InterfaceClass {
            class: interface.class(),
            subclass: interface.subclass(),
            protocol: interface.protocol(),
        })
        .collect();
    if interfaces.is_empty() {
        // Some platforms report no interfaces for simple devices; fall back to the device class.
        interfaces.push(InterfaceClass {
            class: info.class(),
            subclass: info.subclass(),
            protocol: info.protocol(),
        });
    }
    UsbDevice {
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        serial: info.serial_number().map(str::to_owned),
        manufacturer: info.manufacturer_string().map(str::to_owned),
        name: info.product_string().map_or_else(
            || format!("{:04x}:{:04x}", info.vendor_id(), info.product_id()),
            str::to_owned,
        ),
        kinds: DeviceKinds::from_interfaces(interfaces),
    }
}
