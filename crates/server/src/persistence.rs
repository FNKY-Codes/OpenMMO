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
