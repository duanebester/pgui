//! SSH Tunnel Test Binary
//!
//! Tests SSH tunneling to PostgreSQL using the SshTunnel service.
//!
//! Prerequisites:
//!   docker compose -f docker-compose.ssh-test.yml up -d
//!
//! Run with:
//!   cargo run --bin test_ssh_tunnel

use anyhow::Result;
use pgui::ssh::{SshTunnel, SshTunnelConfig};
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
