//! One kvmux per user. A second launch hands its request to the running copy and exits.

/// What a second `kvmux` asks the running copy: open the settings window.
pub const OPEN_SETTINGS: &str = "settings";
/// A second `kvmux --background` only checks that the running copy is alive.
pub const PING: &str = "ping";

/// The running copy's answer to a request from a second launch. Anything it does not
/// recognise gets no reply, so a stray client cannot make it act.
pub fn handle(request: &str, mut open_settings: impl FnMut()) -> Option<String> {
    match request {
        OPEN_SETTINGS => {
            open_settings();
            Some("ok".to_owned())
        }
        PING => Some("ok".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_request_opens_settings_and_replies() {
        let mut opened = 0;
        assert_eq!(handle(OPEN_SETTINGS, || opened += 1).as_deref(), Some("ok"));
        assert_eq!(opened, 1);
    }

    #[test]
    fn ping_replies_without_opening_anything() {
        let mut opened = 0;
        assert_eq!(handle(PING, || opened += 1).as_deref(), Some("ok"));
        assert_eq!(opened, 0);
    }

    #[test]
    fn unknown_requests_get_no_reply_and_do_nothing() {
        let mut opened = 0;
        assert_eq!(handle("quit", || opened += 1), None);
        assert_eq!(handle("", || opened += 1), None);
        assert_eq!(opened, 0);
    }
}
