# Snap Package

## Building

Make sure `snapcraft` is installed:

```sh
sudo snap install snapcraft --classic
```

Build the snap:

```sh
snapcraft
```

This produces a file like `framework-tool_v0.6.1-20-gabda498ca6_amd64.snap`.

To clean up build artifacts and start fresh:

```sh
snapcraft clean
```

## Installing Locally

Several interfaces (`cros-ec`, `hidraw`) require store assertions or a gadget
snap to provide slots, which are not available on standard desktop systems.
For local testing, install with `--devmode` to bypass confinement:

```sh
sudo snap install --dangerous --devmode framework-tool_*.snap
```

## Connecting Interfaces

See the [Snap Interfaces](../../README.md#snap-interfaces) section in the README
for the full list of interface connect commands (applicable when installed from
the snap store with proper assertions).

Verify the connections:

```sh
snap connections framework-tool
```

## Testing

```sh
# Basic functionality
sudo framework-tool --help
sudo framework-tool --versions
sudo framework-tool --esrt

# EC communication (needs cros-ec + hardware-observe)
sudo framework-tool --power
sudo framework-tool --pdports

# HID devices (needs hidraw)
sudo framework-tool --touchpad-info

# USB devices (needs raw-usb)
sudo framework-tool --dp-hdmi-info
sudo framework-tool --audio-card-info

# NVMe (needs block-devices)
sudo framework-tool --nvme-info
```

If a command fails with a permission error, check which interface it needs
and make sure it is connected.

## Publishing

See: https://snapcraft.io/docs/releasing-your-app

Note: Several interfaces (`block-devices`, `physical-memory-observe`,
`io-ports-control`, `system-files`) are privileged and require a manual
review by the snap store team before they can be used in a published snap.
