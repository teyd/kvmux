# kvmux

Turn a USB switch into a KVM. kvmux watches for a USB device, such as your keyboard or mouse,
to connect and switches your monitors to the right input over DDC/CI. Pick the device and
the inputs in the settings window, and kvmux writes the config for you.

Windows and Linux. Built with [gpui-kit](https://gpui-kit.com).

## Use

Run `kvmux` to open settings. Select a connected USB trigger, enable the displays
to control, and choose their connection inputs. Disconnection inputs are optional;
“Leave unchanged” does not send a command. Save applies the choices to the running
watcher and restores them on the next launch.

Keep the window open while switching. Closing it quits; this MVP does not install
a service, run in the tray, or start automatically. Existing `monitors`, `usb
--watch`, and `set-input` commands remain hardware diagnostics.

Configuration is stored at `$XDG_CONFIG_HOME/kvmux/config.toml` (or
`~/.config/kvmux/config.toml`) on Linux and `%APPDATA%\kvmux\config.toml` on Windows.
Use Tab to navigate, arrow keys and Enter in selectors, Ctrl+S to save (Cmd+S on
macOS), and Ctrl+R to refresh displays. Missing saved devices remain selected so
temporary disconnections do not discard rules.

Displays must report an EDID identity. If two displays report the same identity,
kvmux refuses to switch them rather than guess from discovery order.

## Build

Install [mise](https://mise.jdx.dev), then `mise trust && mise run ci`. Linux also needs the
gpui-kit build libraries:

- Fedora: `sudo dnf install fontconfig-devel libxkbcommon-x11-devel systemd-devel`
- Ubuntu: `sudo apt install libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libvulkan1 libudev-dev`

Windows needs the MSVC toolchain and CMake.

On Linux, switching monitors needs access to `/dev/i2c-*`: load the `i2c-dev` module and add
your user to a group that owns those devices (usually `i2c`).

Licensed under the MIT license.

Inspired by [display-switch](https://github.com/haimgel/display-switch) by Haim Gelfenbeyn.
