# ttypicker <!-- omit from toc -->

**ttypicker** is an async, Rust-based XDG Desktop Portal backend that implements the `FileChooser` interface and runs your terminal file manager as a file picker.

- [Installation](#installation)
  - [1. Executable](#1-executable)
  - [2. Config file](#2-config-file)
  - [3. Systemd service](#3-systemd-service)
  - [4. D-Bus service](#4-d-bus-service)
  - [5. XDG Desktop Portal](#5-xdg-desktop-portal)
- [Advanced Configuration](#advanced-configuration)
- [Debugging](#debugging)
  - [Service logs](#service-logs)
  - [D-Bus messages](#d-bus-messages)
- [For developers](#for-developers)
- [FAQ](#faq)
  - [Why doesn't my app open the file picker?](#why-doesnt-my-app-open-the-file-picker)
- [References](#references)

## Installation

### 1. Executable

Build `ttypicker` from source and install the executable in `~/.local/bin`.
```sh
git clone https://github.com/olibrks/ttypicker
cd ttypicker
cargo build --release
install -Dm755 target/release/ttypicker ~/.local/bin/ttypicker
```
> [!IMPORTANT]
> Run all commands from the root of the cloned repository.
### 2. Config file

A config file should be created as `~/.config/ttypicker/config.toml`.

For example, a standard `config.toml` for `kitty` and `yazi` is:
```toml
[FileChooser]
term="kitty --class yazi --title {TITLE}"
exec="yazi --chooser-file={OUT_PATH} {IN_PATH}"
in_path="{SUGGESTED_PATH}"
```
- The mandatory `term` and `exec` keys define the command launched by `ttypicker`, including the terminal emulator and terminal file manager arguments.

- The `in_path` key defines the input path for the TUI file manager, used with `{IN_PATH}` token in the `exec` command. It creates a unique file path based on the path provided (typically `{SUGGESTED_PATH}` for the app-suggested path or `{LAST_PATH}` for the most recently used folder). If the path exists, it will be suffixed with `_X` (e.g. `existing_file_1.txt`).

See template config files for other terminal file managers in [config](./config/) folder and modify it to your needs.
```sh
install -Dm644 config/config_yazi.toml ~/.config/ttypicker/config.toml
```

<details>
<summary>Check that your config is valid</summary>

```sh
~/.local/bin/ttypicker --check-config
```

This command will perform a basic check of your config file.

</details>

### 3. Systemd service

Install the user-level systemd unit [`ttypicker.service`](./share/ttypicker.service) in `~/.config/systemd/user/`.

Reload the systemd user daemon to register the new service.
```sh
install -Dm644 share/ttypicker.service ~/.config/systemd/user/ttypicker.service
systemctl --user daemon-reload
```

<details>
<summary>Check that the service is loaded</summary>

```sh
systemctl --user status ttypicker
```

The service should show `Loaded: loaded`.

It is normal that it's `inactive (dead)`. The service will be automatically activated by D-Bus.

</details>

### 4. D-Bus service

Install [`org.freedesktop.impl.portal.desktop.ttypicker.service`](./share/org.freedesktop.impl.portal.desktop.ttypicker.service) in `~/.local/share/dbus-1/services/` and reload D-Bus broker to register the user-level D-Bus service.
```sh
install -Dm644 share/org.freedesktop.impl.portal.desktop.ttypicker.service \
  ~/.local/share/dbus-1/services/org.freedesktop.impl.portal.desktop.ttypicker.service
systemctl --user reload dbus
```

<details>
<summary>Check that the D-Bus service is registered</summary>

```sh
busctl --user list --activatable | grep ttypicker
```

The output should contain `org.freedesktop.impl.portal.desktop.ttypicker`.

</details>

### 5. XDG Desktop Portal

Install [`ttypicker.portal`](./share/ttypicker.portal) in `~/.local/share/xdg-desktop-portal/portals/` to register the D-Bus service as a `FileChooser` portal.
```sh
install -Dm644 share/ttypicker.portal ~/.local/share/xdg-desktop-portal/portals/ttypicker.portal
```

Create or update `~/.config/xdg-desktop-portal/portals.conf` to make `ttypicker` the preferred `FileChooser` portal.

```sh
mkdir -p ~/.config/xdg-desktop-portal/
${EDITOR:-vim} ~/.config/xdg-desktop-portal/portals.conf
```
Paste the following lines if the file is empty. If it already has a `[preferred]` section, only add the second line to it.
```ini
[preferred]
org.freedesktop.impl.portal.FileChooser=ttypicker
```

Restart `XDG Desktop Portal` service to take into account the new configuration.
```sh
systemctl --user restart xdg-desktop-portal
```

<details>
<summary>Check that the portal is running</summary>

```sh
systemctl --user status xdg-desktop-portal
```

The service should show `Active: active (running)`.

</details>

## Advanced Configuration

Power users can modify the configuration file based on additional tokens. See [Configuration](./doc/configuration.md) for advanced features.

## Debugging

### Service logs

<details>
<summary>Raise log level by editing the systemd service</summary>

```sh
systemctl --user edit ttypicker
```
Add the following lines in the opened editor:
```ini
[Service]
Environment="RUST_LOG=trace"
```

Restart the service:
```sh
systemctl --user restart ttypicker
```

**NOTE:** To restore the default log level, run `systemctl --user revert ttypicker` and restart the service.

</details>

Follow the service log:
```sh
journalctl --user -u ttypicker -f
```

### D-Bus messages

Follow the FileChooser request from your app and `ttypicker` service response:
```sh
dbus-monitor --session \
  "interface='org.freedesktop.portal.FileChooser'" \
  "interface='org.freedesktop.portal.Request'"
```

## For developers

`ttypicker` exposes the content of the D-Bus request in a temporary JSON file. Developers can use it to integrate additional features in the file manager such as displaying information about the request, and asking confirmation before overwriting files. See [Integration](./doc/integration.md) for details.

As a basic example, [filechooser.yazi](https://github.com/olibrks/filechooser.yazi) is a plugin adding a file picker header and overwrite confirmation dialog for `yazi`.

## FAQ

### Why doesn't my app open the file picker?

See the [Debugging](#debugging) section to check the service logs and monitor the `FileChooser` request sent by your app and the `ttypicker` response.

Even if `xdg-desktop-portal` is properly configured and running, many apps don't trigger XDG Desktop Portal FileChooser. Some apps like `Blender` use internal picker dialogs. For other apps, it depends on the package manager and toolkit (GTK, Qt, Electron,...).

## References

- [XDG Desktop Portal File Chooser API Reference](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html#)

- Forks of `xdg-desktop-portal-termfilechooser`: [boydaihungst](https://github.com/boydaihungst/xdg-desktop-portal-termfilechooser), [hunkyburrito](https://github.com/hunkyburrito/xdg-desktop-portal-termfilechooser)

