use std::thread::sleep;
use std::time::Duration;

use ddc_hi::{Ddc as _, Display};

use super::id::{EdidFields, edid_id, make_unique};
use super::{DisplayError, DisplaySource, MonitorInfo};
use crate::input::Input;

/// VCP feature code for "input source".
const INPUT_SOURCE: u8 = 0x60;
/// DDC/CI is slow and drops commands, so reads and writes get a few tries.
const ATTEMPTS: u32 = 4;
const PAUSE: Duration = Duration::from_millis(60);

/// Monitors reached over DDC/CI: i2c-dev on Linux, the Monitor Configuration API
/// (and NVAPI) on Windows, IOKit on macOS.
#[derive(Debug, Default)]
pub struct DdcDisplays;

impl DdcDisplays {
    pub fn new() -> Self {
        Self
    }
}

impl DisplaySource for DdcDisplays {
    fn monitors(&mut self) -> Vec<MonitorInfo> {
        let mut displays = Display::enumerate();
        let ids = unique_ids(&displays);
        displays
            .iter_mut()
            .zip(ids)
            .map(|(display, id)| {
                // Capabilities list the supported inputs. Some monitors never answer.
                let _ = display.update_capabilities();
                let current_input = retry(ATTEMPTS, PAUSE, || {
                    display.handle.get_vcp_feature(INPUT_SOURCE)
                })
                .ok()
                // The input is in the low byte; the high byte is reserved.
                .map(|value| Input::from_vcp_value((value.value() & 0xff) as u8));
                MonitorInfo {
                    id,
                    label: label(display),
                    current_input,
                    supported_inputs: supported_inputs(display),
                }
            })
            .collect()
    }

    fn set_input(&mut self, monitor_id: &str, input: Input) -> Result<(), DisplayError> {
        let mut displays = Display::enumerate();
        let ids = unique_ids(&displays);
        let display = displays
            .iter_mut()
            .zip(ids)
            .find_map(|(display, id)| (id == monitor_id).then_some(display))
            .ok_or_else(|| DisplayError::NotFound(monitor_id.to_owned()))?;
        retry(ATTEMPTS, PAUSE, || {
            display
                .handle
                .set_vcp_feature(INPUT_SOURCE, u16::from(input.vcp_value()))
        })
        .map_err(|error| DisplayError::Ddc(error.to_string()))
    }
}

fn unique_ids(displays: &[Display]) -> Vec<String> {
    let mut ids: Vec<String> = displays.iter().map(stable_id).collect();
    make_unique(&mut ids);
    ids
}

fn stable_id(display: &Display) -> String {
    let info = &display.info;
    let fields = EdidFields {
        manufacturer: info.manufacturer_id.clone(),
        product_code: info.model_id,
        serial_text: info.serial_number.clone(),
        serial_number: info.serial,
    };
    // No EDID: the backend's own id is all there is. It can change when cables move.
    edid_id(&fields).unwrap_or_else(|| format!("bus:{}:{}", info.backend, info.id))
}

fn label(display: &Display) -> String {
    let info = &display.info;
    match (&info.manufacturer_id, &info.model_name) {
        (_, Some(model)) => model.clone(),
        (Some(manufacturer), None) => format!("{manufacturer} monitor"),
        (None, None) => format!("Monitor {}", info.id),
    }
}

fn supported_inputs(display: &Display) -> Vec<Input> {
    use mccs_db::ValueType;
    let Some(descriptor) = display.info.mccs_database.get(INPUT_SOURCE) else {
        return Vec::new();
    };
    match &descriptor.ty {
        ValueType::NonContinuous { values, .. } => values
            .keys()
            .map(|value| Input::from_vcp_value(*value))
            .collect(),
        _ => Vec::new(),
    }
}

fn retry<T, E: std::fmt::Display>(
    attempts: u32,
    pause: Duration,
    mut operation: impl FnMut() -> Result<T, E>,
) -> Result<T, E> {
    let mut attempt = 1;
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if attempt >= attempts => return Err(error),
            Err(_) => {
                attempt += 1;
                sleep(pause);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_until_it_works() {
        let mut calls = 0;
        let result = retry(4, Duration::ZERO, || {
            calls += 1;
            if calls < 3 { Err("busy") } else { Ok(calls) }
        });
        assert_eq!(result, Ok(3));
    }

    #[test]
    fn gives_up_after_the_last_attempt() {
        let mut calls = 0;
        let result: Result<(), _> = retry(3, Duration::ZERO, || {
            calls += 1;
            Err(format!("failure {calls}"))
        });
        assert_eq!(result, Err("failure 3".to_owned()));
    }
}
