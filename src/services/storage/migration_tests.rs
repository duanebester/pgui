//! Integration tests for the SQLite storage layer.
//!
//! These tests touch the on-disk SQLite database (in a `tempfile` dir)
//! and use the keyring **mock** backend so they never write to the
//! real macOS Keychain / freedesktop secret service. Each test sets
//! up its own `AppStore` against a fresh path; we never call
//! `AppStore::singleton()` here so the global lock isn't poisoned for
//! other tests.
//!
//! The keyring mock is process-global, so tests in this module use
//! distinct UUIDs/connection names to avoid cross-test interference.
//! That is sufficient because each `Entry` is keyed by `(service, key)`
//! and `key` is the connection UUID we generate per test.
//!
//! Note on parallelism: `cargo test` runs each `#[test]` on a thread.
//! Our installer for the in-memory keyring backend is idempotent and
//! guarded by a `std::sync::Once`.
//!
//! Why not the stock `keyring::mock`? In keyring 3 the bundled mock
//! produces a *fresh* credential per `Entry::new()` call, so a value
//! written through one Entry can't be read through another — which is
//! the exact pattern this codebase uses (write at create-time, read
//! later via a separate `Entry`). We bring our own tiny in-memory
//! builder backed by a process-wide `HashMap`, which models the
//! real-store contract: any two entries with the same (service, user)
//! see the same secret.
//!
//! What we cover here:
//! - First-time schema initialization on a fresh file.
//! - Migration from a pre-MySQL/SSH schema (only the original 7 columns)
//!   onto the current schema, including idempotency on re-run.
//! - Round-tripping a Postgres connection (no SSH) through the repo.
//! - Round-tripping a MySQL + SSH (key-file) connection through the repo.
//! - Renames / deletes / `exists_by_name` semantics.
//! - Updating a connection through CRUD.
//! - SSH key passphrase keyring helpers.
//!
//! What we deliberately don't cover here:
//! - Live database connections (PG, MySQL) — that requires Docker and
//!   belongs in a manual smoke-test or a CI integration job.
//! - The SSH tunnel itself — that needs an SSH server and is also
//!   manual smoke-test territory.
use std::any::Any;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex, Once, OnceLock};

use keyring::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

use super::connections::ConnectionsRepository;
use super::types::{ConnectionInfo, DatabaseDriver, SslMode};
use super::AppStore;
use crate::services::ssh::{SshAuth, SshConfig};

// =====================================================================
// In-memory keyring backend (process-wide)
// =====================================================================

fn store() -> &'static Mutex<HashMap<(String, String), Vec<u8>>> {
    static STORE: OnceLock<Mutex<HashMap<(String, String), Vec<u8>>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

struct InMemoryCredential {
    service: String,
    user: String,
}

impl CredentialApi for InMemoryCredential {
    fn set_secret(&self, secret: &[u8]) -> keyring::Result<()> {
        store()
            .lock()
            .unwrap()
            .insert((self.service.clone(), self.user.clone()), secret.to_vec());
        Ok(())
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        store()
            .lock()
            .unwrap()
            .get(&(self.service.clone(), self.user.clone()))
            .cloned()
            .ok_or(keyring::Error::NoEntry)
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        let removed = store()
            .lock()
            .unwrap()
            .remove(&(self.service.clone(), self.user.clone()));
        match removed {
            Some(_) => Ok(()),
            None => Err(keyring::Error::NoEntry),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct InMemoryBuilder;

impl CredentialBuilderApi for InMemoryBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(InMemoryCredential {
            service: service.to_string(),
            user: user.to_string(),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

static KEYRING_INIT: Once = Once::new();

/// Install the in-memory keyring backend once per process.
fn init_keyring_mock() {
    KEYRING_INIT.call_once(|| {
        keyring::set_default_credential_builder(Box::new(InMemoryBuilder) as Box<CredentialBuilder>);
    });
}

/// Return `(temp_dir, store)`. The temp dir must be held by the caller
/// to keep the SQLite file alive for the duration of the test.
async fn fresh_store() -> (TempDir, AppStore) {
    init_keyring_mock();
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("pgui.db");
    let store = AppStore::from_path(db_path).await.unwrap();
    (dir, store)
}

/// Build a SQLite pool against `path` without going through `AppStore`,
/// so we can simulate older databases that lack the new columns.
async fn raw_pool(path: &std::path::Path) -> SqlitePool {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .unwrap()
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

#[test]
fn fresh_database_has_all_columns() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;

        // Probe each new column by selecting it; if any is missing the
        // query errors and the test fails.
        for col in [
            "id",
            "name",
            "driver",
            "hostname",
            "username",
            "database",
            "port",
            "ssl_mode",
            "ssh_enabled",
            "ssh_host",
            "ssh_port",
            "ssh_username",
            "ssh_auth_type",
            "ssh_key_path",
        ] {
            let sql = format!("SELECT {} FROM connections LIMIT 1", col);
            sqlx::query(&sql)
                .fetch_optional(&store.pool)
                .await
                .unwrap_or_else(|e| panic!("missing column {}: {}", col, e));
        }
    });
}

#[test]
fn migration_from_legacy_schema_adds_all_columns() {
    smol::block_on(async {
        init_keyring_mock();
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy.db");

        // 1. Create a legacy-shaped table (the schema as it was before
        //    this PR), populate one row, then close the pool.
        {
            let pool = raw_pool(&db_path).await;
            sqlx::query(
                r#"
                CREATE TABLE connections (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL UNIQUE,
                    hostname TEXT NOT NULL,
                    username TEXT NOT NULL,
                    database TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    ssl_mode TEXT NOT NULL DEFAULT 'prefer'
                )
                "#,
            )
            .execute(&pool)
            .await
            .unwrap();

            sqlx::query(
                r#"INSERT INTO connections
                   (id, name, hostname, username, database, port, ssl_mode)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
            )
            .bind("00000000-0000-0000-0000-000000000001")
            .bind("legacy-pg")
            .bind("db.example.com")
            .bind("alice")
            .bind("appdb")
            .bind(5432_i64)
            .bind("prefer")
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;
        }

        // 2. Open via AppStore — initialize_schema is a no-op (table
        //    exists) but migrate_schema must add the new columns.
        let store = AppStore::from_path(db_path.clone()).await.unwrap();

        // 3. All new columns are queryable.
        for col in [
            "driver",
            "ssh_enabled",
            "ssh_host",
            "ssh_port",
            "ssh_username",
            "ssh_auth_type",
            "ssh_key_path",
        ] {
            let sql = format!("SELECT {} FROM connections LIMIT 1", col);
            sqlx::query(&sql)
                .fetch_optional(&store.pool)
                .await
                .unwrap_or_else(|e| panic!("post-migration missing column {}: {}", col, e));
        }

        // 4. Legacy row is loadable, defaults are filled in.
        let conns = store.connections().load_all().await.unwrap();
        assert_eq!(conns.len(), 1);
        let c = &conns[0];
        assert_eq!(c.name, "legacy-pg");
        assert_eq!(c.driver, DatabaseDriver::Postgres, "driver default");
        assert!(c.ssh.is_none(), "legacy row should have no SSH");
        assert_eq!(c.port, 5432);
    });
}

#[test]
fn migration_is_idempotent() {
    smol::block_on(async {
        let (dir, store1) = fresh_store().await;
        let path = dir.path().join("pgui.db");
        // Drop store1's pool and reopen — migrate_schema will run again.
        drop(store1);
        let store2 = AppStore::from_path(path.clone()).await.unwrap();
        // And again, just to be sure.
        drop(store2);
        let store3 = AppStore::from_path(path).await.unwrap();

        // Still queryable, still empty.
        let conns = store3.connections().load_all().await.unwrap();
        assert!(conns.is_empty());
    });
}

#[test]
fn create_load_postgres_no_ssh_roundtrip() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let info = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "pg-direct".to_string(),
            driver: DatabaseDriver::Postgres,
            hostname: "localhost".to_string(),
            username: "alice".to_string(),
            password: "supersecret".to_string(),
            database: "appdb".to_string(),
            port: 5432,
            ssl_mode: SslMode::Require,
            ssh: None,
        };
        repo.create(&info).await.unwrap();

        // load_all returns rows with empty passwords (loaded on-demand).
        let loaded = repo.load_all().await.unwrap();
        assert_eq!(loaded.len(), 1);
        let l = &loaded[0];
        assert_eq!(l.id, info.id);
        assert_eq!(l.name, info.name);
        assert_eq!(l.driver, DatabaseDriver::Postgres);
        assert_eq!(l.port, 5432);
        assert_eq!(l.ssl_mode, SslMode::Require);
        assert!(l.ssh.is_none());
        assert_eq!(l.password, "", "password loaded on-demand, not eagerly");

        // The keyring (mock) does have the password.
        let pw = ConnectionsRepository::get_connection_password(&info.id).unwrap();
        assert_eq!(pw, "supersecret");
    });
}

#[test]
fn create_load_mysql_with_ssh_keyfile_roundtrip() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let info = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "mysql-via-bastion".to_string(),
            driver: DatabaseDriver::MySql,
            hostname: "10.0.0.42".to_string(),
            username: "app".to_string(),
            password: "app-pass".to_string(),
            database: "appdb".to_string(),
            port: 3306,
            ssl_mode: SslMode::Prefer,
            ssh: Some(SshConfig {
                host: "bastion.internal".to_string(),
                port: 2222,
                username: "deploy".to_string(),
                auth: SshAuth::KeyFile {
                    path: "/Users/me/.ssh/id_ed25519".to_string(),
                },
            }),
        };
        repo.create(&info).await.unwrap();

        let loaded = repo.load_all().await.unwrap();
        assert_eq!(loaded.len(), 1);
        let l = &loaded[0];
        assert_eq!(l.driver, DatabaseDriver::MySql);
        assert_eq!(l.port, 3306);
        let ssh = l.ssh.as_ref().expect("ssh should be present");
        assert_eq!(ssh.host, "bastion.internal");
        assert_eq!(ssh.port, 2222);
        assert_eq!(ssh.username, "deploy");
        match &ssh.auth {
            SshAuth::KeyFile { path } => assert_eq!(path, "/Users/me/.ssh/id_ed25519"),
            other => panic!("unexpected auth: {:?}", other),
        }
    });
}

#[test]
fn create_load_mysql_with_ssh_agent() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let info = ConnectionInfo {
            id: Uuid::new_v4(),
            name: "mysql-agent".to_string(),
            driver: DatabaseDriver::MySql,
            hostname: "db.private".to_string(),
            username: "ro".to_string(),
            password: "ro-pass".to_string(),
            database: "metrics".to_string(),
            port: 3306,
            ssl_mode: SslMode::Disable,
            ssh: Some(SshConfig {
                host: "jump.example.com".to_string(),
                port: 22,
                username: "ops".to_string(),
                auth: SshAuth::Agent,
            }),
        };
        repo.create(&info).await.unwrap();

        let loaded = &repo.load_all().await.unwrap()[0];
        let ssh = loaded.ssh.as_ref().unwrap();
        assert!(matches!(ssh.auth, SshAuth::Agent));
    });
}

#[test]
fn duplicate_name_is_rejected_on_create() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let mut a = ConnectionInfo::default();
        a.id = Uuid::new_v4();
        a.name = "dup".to_string();
        repo.create(&a).await.unwrap();

        let mut b = ConnectionInfo::default();
        b.id = Uuid::new_v4();
        b.name = "dup".to_string();
        let err = repo.create(&b).await.unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "expected already-exists error, got: {}",
            err
        );
    });
}

#[test]
fn update_changes_driver_and_ssh_fields() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let id = Uuid::new_v4();
        let mut info = ConnectionInfo {
            id,
            name: "evolves".to_string(),
            driver: DatabaseDriver::Postgres,
            hostname: "h".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            database: "d".to_string(),
            port: 5432,
            ssl_mode: SslMode::Prefer,
            ssh: None,
        };
        repo.create(&info).await.unwrap();

        // Mutate: switch to MySQL + add an SSH agent tunnel.
        info.driver = DatabaseDriver::MySql;
        info.port = 3306;
        info.ssh = Some(SshConfig {
            host: "ssh.example".to_string(),
            port: 22,
            username: "me".to_string(),
            auth: SshAuth::Agent,
        });
        repo.update(&info).await.unwrap();

        let loaded = repo.load_all().await.unwrap();
        let l = &loaded[0];
        assert_eq!(l.driver, DatabaseDriver::MySql);
        assert_eq!(l.port, 3306);
        let ssh = l.ssh.as_ref().unwrap();
        assert_eq!(ssh.host, "ssh.example");
        assert!(matches!(ssh.auth, SshAuth::Agent));

        // Now drop SSH back to None and verify the row reflects that.
        info.ssh = None;
        repo.update(&info).await.unwrap();
        let l2 = &repo.load_all().await.unwrap()[0];
        assert!(l2.ssh.is_none());
    });
}

#[test]
fn delete_removes_row_and_password() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let id = Uuid::new_v4();
        let mut info = ConnectionInfo::default();
        info.id = id;
        info.name = "to-be-deleted".to_string();
        info.password = "ephemeral".to_string();
        repo.create(&info).await.unwrap();

        // Sanity: password is in mock keyring.
        assert_eq!(
            ConnectionsRepository::get_connection_password(&id).unwrap(),
            "ephemeral"
        );

        repo.delete(&id).await.unwrap();

        // Row gone.
        assert!(repo.load_all().await.unwrap().is_empty());
        // Password gone (mock keyring returns NoEntry).
        assert!(ConnectionsRepository::get_connection_password(&id).is_err());
    });
}

#[test]
fn ssh_key_passphrase_roundtrip_via_keyring() {
    init_keyring_mock();
    let id = Uuid::new_v4();

    // Initially: nothing stored.
    assert!(ConnectionsRepository::get_ssh_key_passphrase(&id).is_none());

    // Store, then read back.
    ConnectionsRepository::store_ssh_key_passphrase(&id, "hunter2").unwrap();
    assert_eq!(
        ConnectionsRepository::get_ssh_key_passphrase(&id).as_deref(),
        Some("hunter2")
    );

    // Empty string clears it.
    ConnectionsRepository::store_ssh_key_passphrase(&id, "").unwrap();
    assert!(ConnectionsRepository::get_ssh_key_passphrase(&id).is_none());
}

#[test]
fn exists_by_name_is_case_sensitive_and_scoped() {
    smol::block_on(async {
        let (_dir, store) = fresh_store().await;
        let repo = store.connections();

        let mut info = ConnectionInfo::default();
        info.id = Uuid::new_v4();
        info.name = "Prod".to_string();
        repo.create(&info).await.unwrap();

        assert!(repo.exists_by_name("Prod").await.unwrap());
        // Case-sensitive: SQLite's default `=` on TEXT is binary-collation.
        assert!(!repo.exists_by_name("prod").await.unwrap());
        assert!(!repo.exists_by_name("Staging").await.unwrap());
    });
}
