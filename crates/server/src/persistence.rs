use openmmo_common::PlayerState;
use openmmo_protocol::ServerMessage;

use crate::state::GameWorld;

/// PostgreSQL persistence layer (optional via `postgres` feature).
/// In-memory mode is used for local dev and tests.

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
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS audit_log (
    id SERIAL PRIMARY KEY,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
"#;

pub async fn run_migrations(database_url: &str) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = database_url;
    }
    Ok(())
}

pub async fn save_player(database_url: &str, player: &PlayerState) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        let data = serde_json::to_value(player)?;
        sqlx::query(
            r#"
            INSERT INTO characters (id, name, data)
            VALUES ($1, $2, $3)
            ON CONFLICT (name) DO UPDATE SET data = EXCLUDED.data
            "#,
        )
        .bind(player.id.0)
        .bind(&player.name)
        .bind(data)
        .execute(&pool)
        .await?;
        return Ok(());
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = (database_url, player);
        Ok(())
    }
}

pub async fn load_or_create_player(
    database_url: &str,
    username: &str,
    character_name: &str,
    world: &mut GameWorld,
) -> anyhow::Result<openmmo_common::PlayerId> {
    #[cfg(feature = "postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM characters WHERE name = $1")
                .bind(character_name)
                .fetch_optional(&pool)
                .await?;
        if let Some((data,)) = row {
            let mut player: PlayerState = serde_json::from_value(data)?;
            player.name = character_name.to_string();
            let id = player.id;
            let entity_id = world.alloc_entity();
            player.entity_id = entity_id;
            world.players.insert(id, player);
            return Ok(id);
        }
        let _ = username;
        Ok(world.add_player(character_name.to_string()))
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = (database_url, username);
        Ok(world.add_player(character_name.to_string()))
    }
}

pub async fn persist_audit(database_url: &str, message: &str) -> anyhow::Result<()> {
    #[cfg(feature = "postgres")]
    {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await?;
        sqlx::query("INSERT INTO audit_log (message) VALUES ($1)")
            .bind(message)
            .execute(&pool)
            .await?;
    }
    #[cfg(not(feature = "postgres"))]
    {
        let _ = (database_url, message);
    }
    Ok(())
}
