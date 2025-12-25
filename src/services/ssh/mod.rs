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

pub use askpass::{AskpassProxy, handle_askpass_mode};
pub use reconnect::{ExponentialBackoff, ReconnectConfig};
pub use service::{SshService, TunnelId, TunnelState};
pub use tunnel::SshTunnel;
pub use types::{SshAuthMethod, SshTunnelConfig};
