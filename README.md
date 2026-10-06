# kvmux

Turn a USB switch into a KVM. kvmux watches for a USB device, such as your keyboard or mouse,
to connect and switches your monitors to the right input over DDC/CI. Pick the device and
the inputs in the settings window, and kvmux writes the config for you.

Windows and Linux. Built with [gpui-kit](https://gpui-kit.com).

## Build

Install [mise](https://mise.jdx.dev), then `mise trust && mise run ci`. Linux also needs the
gpui-kit build libraries:

- Fedora: `sudo dnf install fontconfig-devel libxkbcommon-x11-devel`
- Ubuntu: `sudo apt install libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev libzstd-dev libvulkan1`

Windows needs the MSVC toolchain and CMake.

Licensed under the MIT license.

Inspired by [display-switch](https://github.com/haimgel/display-switch) by Haim Gelfenbeyn.
