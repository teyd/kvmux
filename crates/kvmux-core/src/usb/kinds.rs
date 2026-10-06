/// What a device is, judged from its interfaces. A wireless receiver often is both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceKinds {
    pub keyboard: bool,
    pub mouse: bool,
}

/// The class triple of one USB interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceClass {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

const HID_CLASS: u8 = 0x03;
const BOOT_SUBCLASS: u8 = 0x01;
const PROTOCOL_KEYBOARD: u8 = 0x01;
const PROTOCOL_MOUSE: u8 = 0x02;

impl DeviceKinds {
    /// A keyboard or mouse has an HID interface with the boot subclass and the matching
    /// protocol. Composite devices are checked interface by interface. Gaming keyboards that
    /// only expose non-boot HID interfaces are not recognised; they are still listed as
    /// other devices, so the user can pick them.
    pub fn from_interfaces(interfaces: impl IntoIterator<Item = InterfaceClass>) -> Self {
        let mut kinds = Self::default();
        for interface in interfaces {
            if interface.class == HID_CLASS && interface.subclass == BOOT_SUBCLASS {
                match interface.protocol {
                    PROTOCOL_KEYBOARD => kinds.keyboard = true,
                    PROTOCOL_MOUSE => kinds.mouse = true,
                    _ => {}
                }
            }
        }
        kinds
    }

    pub fn is_peripheral(self) -> bool {
        self.keyboard || self.mouse
    }

    pub fn label(self) -> &'static str {
        match (self.keyboard, self.mouse) {
            (true, true) => "Keyboard and mouse",
            (true, false) => "Keyboard",
            (false, true) => "Mouse",
            (false, false) => "Other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interface(class: u8, subclass: u8, protocol: u8) -> InterfaceClass {
        InterfaceClass {
            class,
            subclass,
            protocol,
        }
    }

    #[test]
    fn boot_keyboard() {
        let kinds = DeviceKinds::from_interfaces([interface(3, 1, 1)]);
        assert_eq!(
            kinds,
            DeviceKinds {
                keyboard: true,
                mouse: false
            }
        );
        assert_eq!(kinds.label(), "Keyboard");
    }

    #[test]
    fn boot_mouse() {
        let kinds = DeviceKinds::from_interfaces([interface(3, 1, 2)]);
        assert_eq!(
            kinds,
            DeviceKinds {
                keyboard: false,
                mouse: true
            }
        );
    }

    #[test]
    fn receiver_with_both_is_both() {
        let kinds = DeviceKinds::from_interfaces([
            interface(3, 1, 1),
            interface(3, 1, 2),
            interface(3, 0, 0),
        ]);
        assert_eq!(kinds.label(), "Keyboard and mouse");
        assert!(kinds.is_peripheral());
    }

    #[test]
    fn non_boot_hid_and_other_classes_are_other() {
        let kinds = DeviceKinds::from_interfaces([
            interface(3, 0, 0),
            interface(9, 0, 0),
            interface(1, 1, 1),
        ]);
        assert_eq!(kinds, DeviceKinds::default());
        assert_eq!(kinds.label(), "Other");
        assert!(!kinds.is_peripheral());
    }

    #[test]
    fn protocol_without_boot_subclass_does_not_count() {
        let kinds = DeviceKinds::from_interfaces([interface(3, 0, 1)]);
        assert!(!kinds.is_peripheral());
    }

    #[test]
    fn no_interfaces_is_other() {
        assert_eq!(DeviceKinds::from_interfaces([]), DeviceKinds::default());
    }
}
