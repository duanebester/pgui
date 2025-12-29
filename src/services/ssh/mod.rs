//! SSH tunneling service for secure database connections.
//!
//! This module provides:
//! - `SshTunnel` - Low-level SSH tunnel using system ssh binary
//! - `SshService` - High-level GPUI-integrated tunnel management
//! - `AskpassProxy` - Secure password delivery via Unix socket

mod askpass;
mod reconnect;
mod service;
mod tunnel;
mod types;

#[allow(unused_imports)]
pub use askpass::{AskpassProxy, handle_askpass_mode};
#[allow(unused_imports)]
pub use reconnect::{ExponentialBackoff, ReconnectConfig};
pub use service::{SshService, TunnelId, TunnelState};
#[allow(unused_imports)]
pub use tunnel::SshTunnel;
pub use types::{Host, SshAuthMethod, SshTunnelConfig};
