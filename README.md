# PGUI

A high performance GUI to query & manage PostgreSQL and MySQL databases.

Written in [GPUI](https://gpui.rs) with [GPUI Component](https://github.com/longbridge/gpui-component)

<img src="https://github.com/duanebester/pgui/blob/main/assets/screenshots/pgui-dual.png" height="400px" />

### Supported databases

- **PostgreSQL** (any reasonably recent version)
- **MySQL 8.4 LTS** (the wire protocol and `information_schema` queries are
  also compatible with the 8.0 series)

### Connections

Connections and query history are saved to a SQLite database at
`~/.pgui/pgui.db`. The connection form lets you pick a driver per saved
connection; the default port adjusts automatically (5432 for Postgres,
3306 for MySQL).

Database passwords and SSH key passphrases are stored in the host OS
secure store via the Keyring crate, never in the SQLite database.

### SSH tunnels

Any saved connection can be routed through an SSH tunnel. Toggle
**"Connect through SSH tunnel"** in the connection form and provide:

- SSH host / port / user
- Authentication: **SSH Agent** (uses `SSH_AUTH_SOCK`) or a **Private Key
  File** (with optional passphrase saved to the keyring)

pgui opens a local-port-forward tunnel (`127.0.0.1:<random>` →
`<db host>:<db port>` over SSH) and connects sqlx to the local end. The
tunnel is torn down when you disconnect.

Password-based SSH authentication is intentionally not supported; use a
key or an agent.

### Agent Panel

Only Anthropic support w/ `ANTHROPIC_API_KEY` via enviroment.

### AI Completions (Cmd+.)

AI Completions are triggered via code actions (cmd + .) or via the inline completions toggle.

> Note: currently hard-coded to claude haiku 4.5

### Building

See [Mac App Build](./MAC_APP_BUILD.md) for building locally on MacOS
