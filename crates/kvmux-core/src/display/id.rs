/// The parts of a monitor's EDID that identify it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EdidFields {
    /// Three letter PnP manufacturer id, for example `DEL`.
    pub manufacturer: Option<String>,
    pub product_code: Option<u16>,
    /// The text serial from the EDID descriptor, preferred when present.
    pub serial_text: Option<String>,
    /// The numeric serial from the EDID header. Zero means "not set".
    pub serial_number: Option<u32>,
}

/// `DEL:41C3:ABC123`: manufacturer, product code and serial. Stable across reboots,
/// ports and the order the OS lists displays in. `None` when the monitor has no EDID.
pub(crate) fn edid_id(fields: &EdidFields) -> Option<String> {
    let manufacturer = fields.manufacturer.as_deref()?.trim();
    if manufacturer.is_empty() {
        return None;
    }
    let product = fields.product_code.unwrap_or(0);
    let serial = fields
        .serial_text
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            fields
                .serial_number
                .filter(|n| *n != 0)
                .map(|n| format!("{n:08X}"))
        })
        .unwrap_or_default();
    Some(format!(
        "{}:{product:04X}:{serial}",
        manufacturer.to_ascii_uppercase()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(serial_text: Option<&str>, serial_number: Option<u32>) -> EdidFields {
        EdidFields {
            manufacturer: Some("del".into()),
            product_code: Some(0x41c3),
            serial_text: serial_text.map(str::to_owned),
            serial_number,
        }
    }

    #[test]
    fn prefers_the_text_serial() {
        let id = edid_id(&fields(Some("ABC123"), Some(7)));
        assert_eq!(id.as_deref(), Some("DEL:41C3:ABC123"));
    }

    #[test]
    fn falls_back_to_the_numeric_serial() {
        let id = edid_id(&fields(None, Some(0x1234)));
        assert_eq!(id.as_deref(), Some("DEL:41C3:00001234"));
    }

    #[test]
    fn zero_serial_is_empty() {
        assert_eq!(
            edid_id(&fields(None, Some(0))).as_deref(),
            Some("DEL:41C3:")
        );
        assert_eq!(
            edid_id(&fields(Some("  "), None)).as_deref(),
            Some("DEL:41C3:")
        );
    }

    #[test]
    fn no_manufacturer_means_no_id() {
        assert_eq!(edid_id(&EdidFields::default()), None);
    }
}
