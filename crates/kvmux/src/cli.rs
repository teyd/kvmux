//! Command-line helpers for trying the hardware layer without the GUI.

use kvmux_core::{DisplaySource, Input, UsbDevice, UsbError, UsbEvent, UsbSource};

/// One line per monitor: id, label, current input and supported inputs.
pub fn monitors_report(displays: &mut dyn DisplaySource) -> String {
    let monitors = displays.monitors();
    tracing::info!(count = monitors.len(), "Enumerated monitors");
    if monitors.is_empty() {
        return "No DDC/CI monitors found.".to_owned();
    }
    monitors
        .iter()
        .map(|monitor| {
            let current = monitor
                .current_input
                .map_or("unknown".to_owned(), |input| input.to_string());
            let supported = if monitor.supported_inputs.is_empty() {
                "unknown".to_owned()
            } else {
                monitor
                    .supported_inputs
                    .iter()
                    .map(Input::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            format!(
                "{id}\n  name: {label}\n  input: {current}\n  supports: {supported}",
                id = monitor.id,
                label = monitor.label,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line per device, keyboards and mice first.
pub fn usb_report(usb: &mut dyn UsbSource) -> Result<String, String> {
    let mut devices = usb.devices().map_err(|error| error.to_string())?;
    tracing::info!(count = devices.len(), "Enumerated USB devices");
    devices.sort_by_key(|device| !device.kinds.is_peripheral());
    if devices.is_empty() {
        return Ok("No USB devices found.".to_owned());
    }
    Ok(devices
        .iter()
        .map(describe_device)
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Prints every connect and disconnect until the source ends or fails.
pub fn watch_usb(usb: &mut dyn UsbSource, mut print: impl FnMut(String)) -> UsbError {
    loop {
        match usb.wait_event() {
            Ok(UsbEvent::Connected(device)) => {
                tracing::info!(
                    vendor_id = device.vendor_id,
                    product_id = device.product_id,
                    "USB device connected"
                );
                print(format!("+ {}", describe_device(&device)));
            }
            Ok(UsbEvent::Disconnected(device)) => {
                tracing::info!(
                    vendor_id = device.vendor_id,
                    product_id = device.product_id,
                    "USB device disconnected"
                );
                print(format!("- {}", describe_device(&device)));
            }
            Err(error) => return error,
        }
    }
}

fn describe_device(device: &UsbDevice) -> String {
    let serial = device
        .serial
        .as_ref()
        .map_or(String::new(), |serial| format!("  serial {serial}"));
    format!(
        "{:04x}:{:04x}  {}  [{}]{serial}",
        device.vendor_id,
        device.product_id,
        device.name,
        device.kinds.label(),
    )
}

/// Switches one monitor and returns a line to print.
pub fn set_input_report(
    displays: &mut dyn DisplaySource,
    monitor_id: &str,
    input: &str,
) -> Result<String, String> {
    let input: Input = input.parse().map_err(|error| format!("{error}"))?;
    tracing::info!(monitor_id, %input, "Switching monitor input");
    displays
        .set_input(monitor_id, input)
        .map_err(|error| error.to_string())?;
    tracing::info!(monitor_id, %input, "Monitor input switched");
    Ok(format!("{monitor_id}: switched to {input}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvmux_core::{DeviceKinds, FakeDisplays, FakeUsb, MonitorInfo};

    fn displays() -> FakeDisplays {
        FakeDisplays::new(vec![
            MonitorInfo {
                id: "DEL:41C3:ABC".into(),
                label: "Dell".into(),
                current_input: Some(Input::Hdmi1),
                supported_inputs: vec![Input::Hdmi1, Input::DisplayPort1],
            },
            MonitorInfo {
                id: "LEN:1:".into(),
                label: "Lenovo".into(),
                current_input: None,
                supported_inputs: vec![],
            },
        ])
    }

    #[test]
    fn lists_monitors() {
        let report = monitors_report(&mut displays());
        assert!(report.contains(
            "DEL:41C3:ABC\n  name: Dell\n  input: hdmi1\n  supports: hdmi1, displayport1"
        ));
        assert!(report.contains("LEN:1:\n  name: Lenovo\n  input: unknown\n  supports: unknown"));
    }

    #[test]
    fn says_so_when_there_are_none() {
        let report = monitors_report(&mut FakeDisplays::default());
        assert_eq!(report, "No DDC/CI monitors found.");
    }

    #[test]
    fn switches_a_monitor() {
        let mut fake = displays();
        let line = set_input_report(&mut fake, "DEL:41C3:ABC", "displayport1").expect("switches");
        assert_eq!(line, "DEL:41C3:ABC: switched to displayport1");
        assert_eq!(
            fake.switched(),
            [("DEL:41C3:ABC".to_owned(), Input::DisplayPort1)]
        );
    }

    #[test]
    fn reports_bad_input_and_unknown_monitor() {
        let mut fake = displays();
        assert!(set_input_report(&mut fake, "DEL:41C3:ABC", "thunderbolt").is_err());
        assert!(set_input_report(&mut fake, "nope", "hdmi1").is_err());
        assert!(fake.switched().is_empty());
    }

    fn device(product_id: u16, name: &str, keyboard: bool, serial: Option<&str>) -> UsbDevice {
        UsbDevice {
            vendor_id: 0x046d,
            product_id,
            serial: serial.map(str::to_owned),
            manufacturer: None,
            name: name.into(),
            kinds: DeviceKinds {
                keyboard,
                mouse: false,
            },
        }
    }

    #[test]
    fn usb_report_lists_peripherals_first() {
        let mut usb = FakeUsb::new(vec![
            device(1, "Hub", false, None),
            device(2, "Keyboard", true, Some("S1")),
        ]);
        let report = usb_report(&mut usb).expect("lists");
        assert_eq!(
            report,
            "046d:0002  Keyboard  [Keyboard]  serial S1\n046d:0001  Hub  [Other]"
        );
    }

    #[test]
    fn usb_report_says_so_when_empty() {
        assert_eq!(
            usb_report(&mut FakeUsb::default()).as_deref(),
            Ok("No USB devices found.")
        );
    }

    #[test]
    fn watch_prints_events_until_the_stream_ends() {
        let mut usb = FakeUsb::default();
        usb.queue(UsbEvent::Connected(device(2, "Keyboard", true, None)));
        usb.queue(UsbEvent::Disconnected(device(2, "Keyboard", true, None)));
        let mut lines = Vec::new();
        let ended = watch_usb(&mut usb, |line| lines.push(line));
        assert_eq!(ended, UsbError::Closed);
        assert_eq!(
            lines,
            [
                "+ 046d:0002  Keyboard  [Keyboard]",
                "- 046d:0002  Keyboard  [Keyboard]"
            ]
        );
    }
}
