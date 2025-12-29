//! SSH Tunnel Test Binary
//!
//! Tests SSH tunneling to PostgreSQL using both the low-level SshTunnel
//! and the high-level SshService.
//!
//! Prerequisites:
//!   docker compose -f docker-compose.ssh-test.yml up -d
//!
//! Run with:
//!   cargo run --bin test_ssh_tunnel

use anyhow::Result;
use pgui::ssh::{SshService, SshTunnel, SshTunnelConfig, TunnelState, handle_askpass_mode};
use std::time::Duration;

// Test configuration
const SSH_HOST: &str = "127.0.0.1";
const SSH_PORT: u16 = 2222;
const SSH_USER: &str = "testuser";
const SSH_PASS: &str = "testpass";

const PG_REMOTE_HOST: &str = "postgres";
const PG_REMOTE_PORT: u16 = 5432;
const PG_USER: &str = "pguser";
const PG_PASS: &str = "pgpass";
const PG_DB: &str = "testdb";

fn main() -> Result<()> {
    // Handle --askpass mode first (before any other initialization)
    handle_askpass_mode();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("test_ssh_tunnel=debug".parse().unwrap())
                .add_directive("pgui=debug".parse().unwrap())
                .add_directive("warn".parse().unwrap()),
        )
        .init();

    println!("╔════════════════════════════════════════════╗");
    println!("║   SSH Tunnel Test Suite (System SSH)       ║");
    println!("╚════════════════════════════════════════════╝\n");

    smol::block_on(async {
        println!("━━━ Test 1: SSH Connection Test ━━━");
        test_ssh_connection().await?;

        println!("\n━━━ Test 2: Full Tunnel with sqlx ━━━");
        test_tunnel_with_sqlx().await?;

        println!("\n━━━ Test 3: SshService State Management ━━━");
        test_service_state_management().await?;

        println!("\n━━━ Test 4: SshService Keychain ━━━");
        test_service_keychain()?;

        println!("\n━━━ Test 5: SshService Tunnel Lifecycle ━━━");
        test_service_tunnel_lifecycle().await?;

        println!("\n━━━ Test 6: Tunnel Removal (P0 Fix) ━━━");
        test_tunnel_removal().await?;

        println!("\n━━━ Test 7: Askpass Self-Binary (P1 Fix) ━━━");
        test_askpass_self_binary()?;

        println!("\n━━━ Test 8: Multiple Password Prompts ━━━");
        test_multiple_password_serves().await?;

        println!("\n╔════════════════════════════════════════════╗");
        println!("║       All tests passed! ✓                  ║");
        println!("╚════════════════════════════════════════════╝");

        Ok(())
    })
}

async fn test_ssh_connection() -> Result<()> {
    let config = SshTunnelConfig::with_password(
        SSH_HOST,
        SSH_PORT,
        SSH_USER,
        SSH_PASS,
        PG_REMOTE_HOST,
        PG_REMOTE_PORT,
    );

    SshTunnel::test_ssh_connection(&config).await?;

    println!("  ✓ Connected to SSH server at {}:{}", SSH_HOST, SSH_PORT);
    println!("  ✓ Authenticated as user '{}'", SSH_USER);
    Ok(())
}

async fn test_tunnel_with_sqlx() -> Result<()> {
    use sqlx::Row;
    use sqlx::postgres::PgPoolOptions;

    let config = SshTunnelConfig::with_password(
        SSH_HOST,
        SSH_PORT,
        SSH_USER,
        SSH_PASS,
        PG_REMOTE_HOST,
        PG_REMOTE_PORT,
    );

    let tunnel = SshTunnel::start(config).await?;
    println!("  → Tunnel listening on {}", tunnel.local_addr());

    smol::Timer::after(Duration::from_millis(100)).await;

    let connection_string = format!(
        "postgres://{}:{}@{}/{}",
        PG_USER,
        PG_PASS,
        tunnel.local_addr(),
        PG_DB
    );

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&connection_string)
        .await?;

    println!("  ✓ sqlx connected through tunnel!");

    let row: (i32,) = sqlx::query_as("SELECT 1 + 1").fetch_one(&pool).await?;
    println!("  ✓ SELECT 1 + 1 = {}", row.0);

    let row = sqlx::query("SELECT version()").fetch_one(&pool).await?;
    let version: String = row.get(0);
    println!(
        "  ✓ PostgreSQL version: {}",
        version.split(',').next().unwrap_or(&version)
    );

    pool.close().await;
    println!("  → Pool closed");

    tunnel.shutdown().await;
    println!("  ✓ Tunnel closed cleanly");

    Ok(())
}

async fn test_service_state_management() -> Result<()> {
    let service = SshService::new();
    let state_tx = service.state_sender();
    let state_rx = service.subscribe();

    let config = SshTunnelConfig::with_password(
        SSH_HOST,
        SSH_PORT,
        SSH_USER,
        SSH_PASS,
        PG_REMOTE_HOST,
        PG_REMOTE_PORT,
    );

    // Spawn a task to collect state changes
    let state_collector = smol::spawn(async move {
        let mut states = Vec::new();
        // Collect states with a timeout
        loop {
            match smol::future::or(async { state_rx.recv().await.ok() }, async {
                smol::Timer::after(Duration::from_secs(5)).await;
                None
            })
            .await
            {
                Some((_, state)) => {
                    states.push(state.clone());
                    if matches!(
                        state,
                        TunnelState::Connected { .. } | TunnelState::Failed { .. }
                    ) {
                        break;
                    }
                }
                None => break,
            }
        }
        states
    });

    // Create tunnel through service
    let result = SshService::create_tunnel(config, state_tx).await;
    assert!(result.is_ok(), "Tunnel creation should succeed");

    let (tunnel_id, tunnel, _config) = result?;
    println!("  ✓ Tunnel {} created via SshService", tunnel_id);

    // Check collected states
    let states = state_collector.await;
    assert!(!states.is_empty(), "Should have received state updates");

    // Should have Connecting -> Connected
    assert!(
        states.iter().any(|s| matches!(s, TunnelState::Connecting)),
        "Should have Connecting state"
    );
    assert!(
        states
            .iter()
            .any(|s| matches!(s, TunnelState::Connected { .. })),
        "Should have Connected state"
    );
    println!("  ✓ State transitions: Connecting -> Connected");

    // Cleanup
    tunnel.shutdown().await;
    println!("  ✓ Service state management works correctly");

    Ok(())
}

fn test_service_keychain() -> Result<()> {
    let test_host = "test-keychain-host";
    let test_port = 22222_u16;
    let test_user = "test-keychain-user";
    let test_pass = "test-keychain-password-12345";

    // Clean up any existing test entry
    let _ = SshService::delete_password(test_host, test_port, test_user);

    // Test that password doesn't exist initially
    let stored = SshService::get_stored_password(test_host, test_port, test_user);
    assert!(stored.is_none(), "Password should not exist initially");
    println!("  ✓ No password stored initially");

    // Store password
    SshService::store_password(test_host, test_port, test_user, test_pass)?;
    println!("  ✓ Password stored in keychain");

    // Retrieve password
    let retrieved = SshService::get_stored_password(test_host, test_port, test_user);
    assert_eq!(
        retrieved,
        Some(test_pass.to_string()),
        "Password should match"
    );
    println!("  ✓ Password retrieved from keychain");

    // Delete password
    SshService::delete_password(test_host, test_port, test_user)?;
    let after_delete = SshService::get_stored_password(test_host, test_port, test_user);
    assert!(after_delete.is_none(), "Password should be deleted");
    println!("  ✓ Password deleted from keychain");

    Ok(())
}

async fn test_service_tunnel_lifecycle() -> Result<()> {
    let mut service = SshService::new();
    let state_tx = service.state_sender();

    let config = SshTunnelConfig::with_password(
        SSH_HOST,
        SSH_PORT,
        SSH_USER,
        SSH_PASS,
        PG_REMOTE_HOST,
        PG_REMOTE_PORT,
    );

    // Create and register tunnel
    let (tunnel_id, tunnel, config) = SshService::create_tunnel(config, state_tx).await?;
    service.register_tunnel(tunnel_id, tunnel, config);
    println!("  ✓ Tunnel registered with service");

    // Check tunnel state
    let state = service.tunnel_state(tunnel_id);
    assert!(state.is_some(), "Tunnel state should exist");
    assert!(state.unwrap().is_connected(), "Tunnel should be connected");
    println!("  ✓ Tunnel state is Connected");

    // Check local address
    let addr = service.local_addr(tunnel_id);
    assert!(addr.is_some(), "Local address should exist");
    println!("  ✓ Local address: {}", addr.unwrap());

    // Check health
    let healthy = service.is_tunnel_healthy(tunnel_id);
    assert!(healthy, "Tunnel should be healthy");
    println!("  ✓ Tunnel is healthy");

    // Check active tunnels
    let active = service.active_tunnels();
    assert!(
        active.contains(&tunnel_id),
        "Tunnel should be in active list"
    );
    println!("  ✓ Tunnel in active tunnels list");

    // Close tunnel
    service.close_tunnel(tunnel_id).await;
    let state_after = service.tunnel_state(tunnel_id);
    assert!(
        state_after.is_none(),
        "Tunnel should be removed after close"
    );
    println!("  ✓ Tunnel closed and removed from service");

    Ok(())
}

/// Test P0 fix: remove_tunnel returns the tunnel for async shutdown
async fn test_tunnel_removal() -> Result<()> {
    let mut service = SshService::new();
    let state_tx = service.state_sender();

    let config = SshTunnelConfig::with_password(
        SSH_HOST,
        SSH_PORT,
        SSH_USER,
        SSH_PASS,
        PG_REMOTE_HOST,
        PG_REMOTE_PORT,
    );

    // Create and register tunnel
    let (tunnel_id, tunnel, config) = SshService::create_tunnel(config, state_tx).await?;
    let local_addr = tunnel.local_addr();
    service.register_tunnel(tunnel_id, tunnel, config);
    println!("  ✓ Tunnel registered at {}", local_addr);

    // Verify tunnel is active
    assert!(
        service.is_tunnel_healthy(tunnel_id),
        "Tunnel should be healthy"
    );

    // Remove tunnel synchronously (simulates what disconnect() does)
    let removed_tunnel = service.remove_tunnel(tunnel_id);
    assert!(
        removed_tunnel.is_some(),
        "remove_tunnel should return the tunnel"
    );
    println!("  ✓ Tunnel removed from service synchronously");

    // Verify tunnel is no longer in service
    assert!(
        service.tunnel_state(tunnel_id).is_none(),
        "Tunnel should not be in service"
    );
    assert!(
        !service.active_tunnels().contains(&tunnel_id),
        "Tunnel should not be active"
    );
    println!("  ✓ Tunnel no longer tracked by service");

    // Shutdown the removed tunnel asynchronously
    let tunnel = removed_tunnel.unwrap();
    tunnel.shutdown().await;
    println!("  ✓ Removed tunnel shut down asynchronously");

    // Verify SSH process is dead by trying to connect to the local port
    smol::Timer::after(Duration::from_millis(200)).await;
    let connect_result = smol::net::TcpStream::connect(&local_addr).await;
    assert!(
        connect_result.is_err(),
        "Local port should no longer be listening"
    );
    println!("  ✓ SSH process terminated (port {} closed)", local_addr);

    Ok(())
}

/// Test P1 fix: askpass script includes self-binary invocation
fn test_askpass_self_binary() -> Result<()> {
    use pgui::ssh::AskpassProxy;

    smol::block_on(async {
        let proxy = AskpassProxy::new().await?;
        let script_path = proxy.script_path();

        // Read the script content
        let script_content = std::fs::read_to_string(script_path)?;

        // Verify it contains --askpass (self-binary approach)
        assert!(
            script_content.contains("--askpass"),
            "Script should contain --askpass flag for self-binary approach"
        );
        println!("  ✓ Askpass script uses --askpass flag");

        // Verify it has nc fallback
        assert!(
            script_content.contains("nc -U") || script_content.contains("nc"),
            "Script should have nc fallback"
        );
        println!("  ✓ Askpass script has nc fallback");

        // Verify no secrets in script
        assert!(
            !script_content.contains(SSH_PASS),
            "Script should not contain password"
        );
        println!("  ✓ No secrets in askpass script");

        // Verify script is executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::metadata(script_path)?.permissions();
            assert!(perms.mode() & 0o100 != 0, "Script should be executable");
            println!("  ✓ Askpass script is executable");
        }

        Ok(())
    })
}

/// Test that askpass proxy can serve passwords multiple times (P0 fix for proxy lifetime)
async fn test_multiple_password_serves() -> Result<()> {
    use pgui::ssh::AskpassProxy;
    use smol::io::AsyncReadExt;
    use std::sync::Arc;

    let proxy = Arc::new(AskpassProxy::new().await?);
    let socket_path = proxy.script_path().parent().unwrap().join("askpass.sock");
    let password = "test_multi_serve_password";

    println!("  → Testing multiple password serves from same proxy");

    // Serve password multiple times (simulating SSH re-prompts)
    for i in 1..=3 {
        let proxy_clone = Arc::clone(&proxy);
        let password_clone = password.to_string();
        let socket_path_clone = socket_path.clone();

        // Spawn server
        let serve_handle = smol::spawn(async move {
            proxy_clone
                .serve_password_with_timeout(&password_clone, Duration::from_secs(5))
                .await
        });

        // Small delay to ensure server is listening
        smol::Timer::after(Duration::from_millis(50)).await;

        // Connect and read password
        let mut stream = smol::net::unix::UnixStream::connect(&socket_path_clone).await?;
        let mut received = String::new();
        stream.read_to_string(&mut received).await?;

        assert_eq!(received.trim(), password, "Password {} should match", i);

        let served = serve_handle.await?;
        assert!(served, "Password {} should have been served", i);

        println!("  ✓ Password serve #{} successful", i);
    }

    println!("  ✓ Proxy successfully served password 3 times");
    Ok(())
}
