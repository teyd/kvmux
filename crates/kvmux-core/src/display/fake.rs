use super::{DisplayError, DisplaySource, MonitorInfo};
use crate::input::Input;

/// In-memory monitors for tests. Records every accepted `set_input`.
#[derive(Debug, Default)]
pub struct FakeDisplays {
    monitors: Vec<MonitorInfo>,
    switched: Vec<(String, Input)>,
}

impl FakeDisplays {
    pub fn new(monitors: Vec<MonitorInfo>) -> Self {
        Self {
            monitors,
            switched: Vec::new(),
        }
    }

    /// The `(monitor id, input)` pairs that were set, in order.
    pub fn switched(&self) -> &[(String, Input)] {
        &self.switched
    }
}

impl DisplaySource for FakeDisplays {
    fn monitors(&mut self) -> Vec<MonitorInfo> {
        self.monitors.clone()
    }

    fn set_input(&mut self, monitor_id: &str, input: Input) -> Result<(), DisplayError> {
        let monitor = self
            .monitors
            .iter_mut()
            .find(|monitor| monitor.id == monitor_id)
            .ok_or_else(|| DisplayError::NotFound(monitor_id.to_owned()))?;
        monitor.current_input = Some(input);
        self.switched.push((monitor_id.to_owned(), input));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(id: &str) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            label: id.into(),
            current_input: Some(Input::Hdmi1),
            supported_inputs: vec![],
        }
    }

    #[test]
    fn set_input_updates_and_records() {
        let mut displays = FakeDisplays::new(vec![monitor("A"), monitor("B")]);
        displays
            .set_input("B", Input::DisplayPort2)
            .expect("known monitor");
        assert_eq!(displays.switched(), [("B".to_owned(), Input::DisplayPort2)]);
        assert_eq!(
            displays.monitors()[1].current_input,
            Some(Input::DisplayPort2)
        );
        assert_eq!(displays.monitors()[0].current_input, Some(Input::Hdmi1));
    }

    #[test]
    fn unknown_monitor_is_an_error() {
        let mut displays = FakeDisplays::new(vec![monitor("A")]);
        assert_eq!(
            displays.set_input("Z", Input::Hdmi1),
            Err(DisplayError::NotFound("Z".into()))
        );
        assert!(displays.switched().is_empty());
    }
}
