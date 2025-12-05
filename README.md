# PGUI

A high performance GUI to query & manage postgres databases.

Written in [GPUI](https://gpui.rs) with [GPUI Component](https://github.com/longbridge/gpui-component)

<img src="https://github.com/duanebester/pgui/blob/main/assets/screenshots/pgui-dual.png" height="400px" />

### Connections

Connections and query history will be saved to a sqlite db file in `~/.pgui/pgui.db`

Passwords are saved in the host OS secure store via Keyring crate.

### Agent Panel

Only Anthropic support w/ `ANTHROPIC_API_KEY` via enviroment.

See [Mac App Build](./MAC_APP_BUILD.md) for building locally on MacOS
