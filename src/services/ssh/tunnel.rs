//! Local-port-forward SSH tunnel implementation backed by `ssh2`.
//!
//! Threading model:
//! - One dedicated OS thread owns the SSH session and the local TCP
//!   listener. This avoids mixing `ssh2`'s blocking API with the async
//!   runtime used by `sqlx`.
//! - For each accepted local connection the thread opens a `direct-tcpip`
//!   channel and spawns two short-lived threads to bidirectionally copy
//!   bytes between the local socket and the channel.
//! - Dropping the [`SshTunnel`] signals the worker thread to exit and
//!   tears down all resources.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use ssh2::Session;

use super::config::{SshAuth, SshConfig};

/// A live SSH tunnel.
///
/// While this value is held, a local TCP listener on `local_port`
/// transparently forwards all traffic to `remote_host:remote_port`
/// through the SSH session. Drop the value to tear the tunnel down.
pub struct SshTunnel {
    local_port: u16,
    shutdown: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl SshTunnel {
    /// The locally-bound port that callers should connect to.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// Establish a new SSH session and start forwarding.
    ///
    /// `remote_host`/`remote_port` is the target as seen from the SSH
    /// server (typically the database host on its private network).
    /// `passphrase` is only consulted for [`SshAuth::KeyFile`].
    pub fn connect(
        cfg: &SshConfig,
        remote_host: String,
        remote_port: u16,
        passphrase: Option<String>,
    ) -> Result<Self> {
        // Open and authenticate the SSH session synchronously so that
        // connection failures surface immediately to the caller.
        let session = open_session(cfg, passphrase.as_deref())?;

        // Bind a local listener on an ephemeral port.
        let listener = TcpListener::bind("127.0.0.1:0")
            .context("Failed to bind local SSH tunnel listener")?;
        let local_port = listener
            .local_addr()
            .context("Failed to read tunnel listener address")?
            .port();
        // Short accept timeout so the worker can observe shutdown.
        listener
            .set_nonblocking(false)
            .context("Failed to configure tunnel listener")?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_worker = shutdown.clone();
        let remote = (remote_host, remote_port);

        let worker = thread::Builder::new()
            .name(format!("ssh-tunnel:{}", local_port))
            .spawn(move || {
                run_tunnel(listener, session, remote, shutdown_for_worker);
            })
            .context("Failed to spawn SSH tunnel worker thread")?;

        Ok(Self {
            local_port,
            shutdown,
            worker: Some(worker),
        })
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        // Nudge the listener out of `accept` by connecting to ourselves;
        // this is a best-effort kick.
        let _ = TcpStream::connect(("127.0.0.1", self.local_port));
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

fn open_session(cfg: &SshConfig, passphrase: Option<&str>) -> Result<Session> {
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let tcp = TcpStream::connect(&addr)
        .with_context(|| format!("Failed to connect to SSH server at {}", addr))?;
    tcp.set_read_timeout(Some(Duration::from_secs(30)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut session = Session::new().context("Failed to create SSH session")?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .context("SSH handshake failed")?;

    match &cfg.auth {
        SshAuth::KeyFile { path } => {
            let key_path = Path::new(path);
            if !key_path.exists() {
                return Err(anyhow!("SSH private key not found: {}", path));
            }
            session
                .userauth_pubkey_file(&cfg.username, None, key_path, passphrase)
                .with_context(|| {
                    format!(
                        "SSH key authentication failed for user '{}' using '{}'",
                        cfg.username, path
                    )
                })?;
        }
        SshAuth::Agent => {
            let mut agent = session
                .agent()
                .context("Failed to access SSH agent")?;
            agent
                .connect()
                .context("Failed to connect to SSH agent (is SSH_AUTH_SOCK set?)")?;
            agent
                .list_identities()
                .context("Failed to list SSH agent identities")?;
            let identities = agent
                .identities()
                .context("Failed to read SSH agent identities")?;
            if identities.is_empty() {
                return Err(anyhow!("SSH agent has no identities loaded"));
            }
            let mut authed = false;
            let mut last_err: Option<ssh2::Error> = None;
            for id in &identities {
                match agent.userauth(&cfg.username, id) {
                    Ok(()) => {
                        authed = true;
                        break;
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            if !authed {
                return Err(match last_err {
                    Some(e) => anyhow!("SSH agent authentication failed: {}", e),
                    None => anyhow!("SSH agent authentication failed"),
                });
            }
        }
    }

    if !session.authenticated() {
        return Err(anyhow!("SSH authentication did not complete"));
    }

    Ok(session)
}

fn run_tunnel(
    listener: TcpListener,
    session: Session,
    remote: (String, u16),
    shutdown: Arc<AtomicBool>,
) {
    // Keep blocking mode on the listener; we use a short accept poll via
    // `set_nonblocking` toggling on shutdown. ssh2 sessions are not Sync,
    // so we serialize channel opens through this thread.
    listener
        .set_nonblocking(true)
        .expect("set_nonblocking on tunnel listener");

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        match listener.accept() {
            Ok((local, _peer)) => {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }
                let (host, port) = (remote.0.clone(), remote.1);
                match session.channel_direct_tcpip(&host, port, None) {
                    Ok(channel) => {
                        if let Err(e) = local.set_nonblocking(false) {
                            tracing::warn!("ssh tunnel: failed to set blocking: {}", e);
                            continue;
                        }
                        spawn_pipes(local, channel);
                    }
                    Err(e) => {
                        tracing::error!(
                            "ssh tunnel: failed to open direct-tcpip to {}:{}: {}",
                            host,
                            port,
                            e
                        );
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                tracing::error!("ssh tunnel: accept failed: {}", e);
                break;
            }
        }
    }

    tracing::debug!("ssh tunnel: worker exiting");
}

fn spawn_pipes(local: TcpStream, channel: ssh2::Channel) {
    // ssh2::Channel is not Send across threads safely for split read/write
    // without care; we wrap it in a single thread that multiplexes both
    // directions using non-blocking I/O.
    thread::Builder::new()
        .name("ssh-tunnel-pipe".into())
        .spawn(move || {
            if let Err(e) = pump(local, channel) {
                tracing::debug!("ssh tunnel: pipe ended: {}", e);
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| tracing::error!("ssh tunnel: spawn pipe failed: {}", e));
}

fn pump(mut local: TcpStream, mut channel: ssh2::Channel) -> std::io::Result<()> {
    local.set_nonblocking(true)?;
    let mut buf_l = [0u8; 16 * 1024];
    let mut buf_r = [0u8; 16 * 1024];
    loop {
        let mut did_work = false;

        // local -> remote
        match local.read(&mut buf_l) {
            Ok(0) => {
                let _ = channel.send_eof();
                break;
            }
            Ok(n) => {
                channel.write_all(&buf_l[..n])?;
                did_work = true;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }

        // remote -> local
        match channel.read(&mut buf_r) {
            Ok(0) => {
                if channel.eof() {
                    break;
                }
            }
            Ok(n) => {
                local.write_all(&buf_r[..n])?;
                did_work = true;
            }
            Err(e) => {
                // ssh2 returns its own error type; treat as fatal here.
                return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
            }
        }

        if !did_work {
            thread::sleep(Duration::from_millis(5));
        }
    }
    let _ = channel.close();
    let _ = channel.wait_close();
    Ok(())
}
