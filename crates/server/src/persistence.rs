// Most of this module only has callers when the `postgres` feature is on.
#![cfg_attr(not(feature = "postgres"), allow(dead_code))]

use openmmo_common::PlayerState;

use crate::state::GameWorld;

/// PostgreSQL persistence layer (optional via `postgres` feature).
///
/// A single connection pool is initialised once at startup with [`init`] and
/// shared process-wide. When no `DATABASE_URL` is configured (or the feature
/// is off) every call is a no-op and the world lives purely in memory — this
/// is the mode used by local dev and unit tests.
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS characters (
    id UUID PRIMARY KEY,
    account_id UUID REFERENCES accounts(id),
    name TEXT UNIQUE NOT NULL,
    data JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

ALTER TABLE characters ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ DEFAULT NOW();
CREATE INDEX IF NOT EXISTS characters_account_id_idx ON characters (account_id);

CREATE TABLE IF NOT EXISTS audit_log (
    id SERIAL PRIMARY KEY,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
"#;

/// Result of an account login attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthResult {
    /// Credentials matched an existing account.
    Ok,
    /// Username was unknown; a new account was created with this password.
    Created,
    /// Username exists but the password did not match.
    BadPassword,
}

/// Why a character could not be attached to an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterError {
    /// The character name belongs to a different account.
    OwnedByOther,
}

#[cfg(feature = "postgres")]
mod pg {
    use std::sync::OnceLock;

    use sqlx::postgres::{PgPool, PgPoolOptions};

    static POOL: OnceLock<PgPool> = OnceLock::new();

    pub fn pool() -> Option<&'static PgPool> {
        POOL.get()
    }

    pub async fn connect(database_url: &str) -> anyhow::Result<()> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect(database_url)
            .await?;
        sqlx::raw_sql(super::SCHEMA).execute(&pool).await?;
        let _ = POOL.set(pool);
        Ok(())
    }
}

/// True when a database is connected and persistence is active.
pub fn enabled() -> bool {
    #[cfg(feature = "postgres")]
    {
        pg::pool().is_some()
    }
    #[cfg(not(feature = "postgres"))]
    {
        false
    }
}

/// Connect to PostgreSQL and apply the schema. Idempotent.
pub async fn init(database_url: &str) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        pg::connect(database_url).await
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = database_url;
        tracing::warn!(
            "DATABASE_URL is set but the server was built without the `postgres` feature; \
             running in memory-only mode"
        );
        Ok(())
    }
}

pub async fn save_player(player: &PlayerState) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let Some(pool) = pg::pool() else {
            return Ok(());
        };
        let data = serde_json::to_value(player)?;
        sqlx::query(
            r#"
            UPDATE characters SET data = $2, updated_at = NOW() WHERE name = $1
            "#,
        )
        .bind(&player.name)
        .bind(data)
        .execute(pool)
        .await?;
        Ok(())
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = player;
        Ok(())
    }
}

/// Save every online player. Errors are logged, not propagated, so one bad
/// row never blocks the rest.
pub async fn save_all(players: &[PlayerState]) -> usize {
    let mut saved = 0;
    for p in players {
        match save_player(p).await {
            Ok(()) => saved += 1,
            Err(e) => tracing::warn!("failed to save {}: {e}", p.name),
        }
    }
    saved
}

pub fn hash_password(password: &str) -> String {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    use argon2::Argon2;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .unwrap_or_default()
}

pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    use argon2::Argon2;
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Verify `username`/`password`, creating the account on first sight.
pub async fn authenticate_account(username: &str, password: &str) -> anyhow::Result<AuthResult> {
    #[cfg(feature = "postgres")]
    {
        let Some(pool) = pg::pool() else {
            return Ok(AuthResult::Ok);
        };
        let row: Option<(String,)> =
            sqlx::query_as("SELECT password_hash FROM accounts WHERE username = $1")
                .bind(username)
                .fetch_optional(pool)
                .await?;
        if let Some((stored,)) = row {
            return Ok(if verify_password(password, &stored) {
                AuthResult::Ok
            } else {
                AuthResult::BadPassword
            });
        }
        sqlx::query("INSERT INTO accounts (id, username, password_hash) VALUES ($1, $2, $3)")
            .bind(uuid::Uuid::new_v4())
            .bind(username)
            .bind(hash_password(password))
            .execute(pool)
            .await?;
        Ok(AuthResult::Created)
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = (username, password);
        Ok(AuthResult::Ok)
    }
}

/// Load `character_name` for `username`, or create it if it does not exist.
///
/// A character is bound to the account that created it; logging in with the
/// same character name from a different account is refused.
pub async fn load_or_create_player(
    username: &str,
    character_name: &str,
    world: &mut GameWorld,
) -> anyhow::Result<Result<openmmo_common::PlayerId, CharacterError>> {
    #[cfg(feature = "postgres")]
    {
        let Some(pool) = pg::pool() else {
            return Ok(Ok(world.add_player(character_name.to_string())));
        };
        let account: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE username = $1")
                .bind(username)
                .fetch_optional(pool)
                .await?;
        let Some((account_id,)) = account else {
            anyhow::bail!("account {username} missing after authentication");
        };

        let row: Option<(Option<uuid::Uuid>, serde_json::Value)> =
            sqlx::query_as("SELECT account_id, data FROM characters WHERE name = $1")
                .bind(character_name)
                .fetch_optional(pool)
                .await?;
        if let Some((owner, data)) = row {
            if owner != Some(account_id) {
                return Ok(Err(CharacterError::OwnedByOther));
            }
            let mut player: PlayerState = serde_json::from_value(data)?;
            player.name = character_name.to_string();
            let id = player.id;
            player.entity_id = world.alloc_entity();
            world.ensure_player_position_valid(&mut player);
            world.players.insert(id, player);
            return Ok(Ok(id));
        }

        let id = world.add_player(character_name.to_string());
        let data = world
            .players
            .get(&id)
            .map(serde_json::to_value)
            .transpose()?
            .unwrap_or(serde_json::Value::Null);
        sqlx::query("INSERT INTO characters (id, account_id, name, data) VALUES ($1, $2, $3, $4)")
            .bind(id.0)
            .bind(account_id)
            .bind(character_name)
            .bind(data)
            .execute(pool)
            .await?;
        Ok(Ok(id))
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = username;
        Ok(Ok(world.add_player(character_name.to_string())))
    }
}

pub async fn persist_audit(message: &str) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let Some(pool) = pg::pool() else {
            return Ok(());
        };
        sqlx::query("INSERT INTO audit_log (message) VALUES ($1)")
            .bind(message)
            .execute(pool)
            .await?;
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = message;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("hunter2");
        assert!(hash.starts_with("$argon2"));
        assert!(verify_password("hunter2", &hash));
        assert!(!verify_password("hunter3", &hash));
    }

    #[test]
    fn verify_rejects_garbage_hash() {
        assert!(!verify_password("x", "not-a-hash"));
        assert!(!verify_password("x", ""));
    }
}
