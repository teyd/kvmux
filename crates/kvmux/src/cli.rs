//! Command-line helpers for trying the hardware layer without the GUI.

use kvmux_core::{DisplaySource, Input};

/// One line per monitor: id, label, current input and supported inputs.
pub fn monitors_report(displays: &mut dyn DisplaySource) -> String {
    let monitors = displays.monitors();
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

/// Switches one monitor and returns a line to print.
pub fn set_input_report(
    displays: &mut dyn DisplaySource,
    monitor_id: &str,
    input: &str,
) -> Result<String, String> {
    let input: Input = input.parse().map_err(|error| format!("{error}"))?;
    displays
        .set_input(monitor_id, input)
        .map_err(|error| error.to_string())?;
    Ok(format!("{monitor_id}: switched to {input}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvmux_core::{FakeDisplays, MonitorInfo};

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
}
