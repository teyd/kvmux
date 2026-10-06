use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A monitor input source, as a DDC/CI VCP feature 0x60 value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Input {
    Vga1,
    Dvi1,
    Dvi2,
    DisplayPort1,
    DisplayPort2,
    Hdmi1,
    Hdmi2,
    /// A value the monitor reports that has no friendly name.
    Raw(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown input {0:?}: use a name like hdmi1, displayport2, or a number like 0x11")]
pub struct ParseInputError(String);

impl Input {
    pub const NAMED: [Input; 7] = [
        Input::Vga1,
        Input::Dvi1,
        Input::Dvi2,
        Input::DisplayPort1,
        Input::DisplayPort2,
        Input::Hdmi1,
        Input::Hdmi2,
    ];

    pub fn vcp_value(self) -> u8 {
        match self {
            Input::Vga1 => 0x01,
            Input::Dvi1 => 0x03,
            Input::Dvi2 => 0x04,
            Input::DisplayPort1 => 0x0f,
            Input::DisplayPort2 => 0x10,
            Input::Hdmi1 => 0x11,
            Input::Hdmi2 => 0x12,
            Input::Raw(value) => value,
        }
    }

    pub fn from_vcp_value(value: u8) -> Self {
        Self::NAMED
            .into_iter()
            .find(|input| input.vcp_value() == value)
            .unwrap_or(Input::Raw(value))
    }

    pub fn label(self) -> String {
        match self {
            Input::Vga1 => "VGA 1".into(),
            Input::Dvi1 => "DVI 1".into(),
            Input::Dvi2 => "DVI 2".into(),
            Input::DisplayPort1 => "DisplayPort 1".into(),
            Input::DisplayPort2 => "DisplayPort 2".into(),
            Input::Hdmi1 => "HDMI 1".into(),
            Input::Hdmi2 => "HDMI 2".into(),
            Input::Raw(value) => format!("Input {value:#04x}"),
        }
    }
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Input::Vga1 => f.write_str("vga1"),
            Input::Dvi1 => f.write_str("dvi1"),
            Input::Dvi2 => f.write_str("dvi2"),
            Input::DisplayPort1 => f.write_str("displayport1"),
            Input::DisplayPort2 => f.write_str("displayport2"),
            Input::Hdmi1 => f.write_str("hdmi1"),
            Input::Hdmi2 => f.write_str("hdmi2"),
            Input::Raw(value) => write!(f, "{value:#04x}"),
        }
    }
}

impl FromStr for Input {
    type Err = ParseInputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = s.trim().to_ascii_lowercase();
        if let Some(named) = Self::NAMED
            .into_iter()
            .find(|input| input.to_string() == text)
        {
            return Ok(named);
        }
        let parsed = match text.strip_prefix("0x") {
            Some(hex) => u8::from_str_radix(hex, 16),
            None => text.parse::<u8>(),
        };
        parsed
            .map(Input::from_vcp_value)
            .map_err(|_| ParseInputError(s.to_owned()))
    }
}

impl Serialize for Input {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Input {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_case_insensitively() {
        assert_eq!("HDMI1".parse(), Ok(Input::Hdmi1));
        assert_eq!(" DisplayPort2 ".parse(), Ok(Input::DisplayPort2));
    }

    #[test]
    fn parses_raw_values_and_maps_known_ones_to_names() {
        assert_eq!("0x11".parse(), Ok(Input::Hdmi1));
        assert_eq!("17".parse(), Ok(Input::Hdmi1));
        assert_eq!("0x1b".parse(), Ok(Input::Raw(0x1b)));
    }

    #[test]
    fn rejects_garbage() {
        assert!("thunderbolt".parse::<Input>().is_err());
        assert!("0x1ff".parse::<Input>().is_err());
        assert!("".parse::<Input>().is_err());
    }

    #[test]
    fn display_round_trips() {
        for input in Input::NAMED.into_iter().chain([Input::Raw(0x1b)]) {
            assert_eq!(input.to_string().parse(), Ok(input));
        }
    }
}
