# PGUI

A high performance GUI to query & manage postgres databases.

Written in [GPUI](https://gpui.rs) with [GPUI Component](https://github.com/longbridge/gpui-component)

### Connections

Connections will be saved to a sqlite db file in `~/.pgui/connections.db`

Passwords are saved in the host OS secure store via Keyring crate.

### Agent Panel

Right now only Anthropic support w/ Claude 4.5 Sonnet via the env var.

As of 2025-11-18:

![screengrab-dark](./assets/screenshots/2025-11-18-dark.png)

![screengrab-light](./assets/screenshots/2025-11-18-light.png)

See [Mac App Build](./MAC_APP_BUILD.md) for building locally on MacOS
