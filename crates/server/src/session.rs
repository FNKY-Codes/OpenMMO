use openmmo_common::PlayerId;

/// Redis-backed session cache: maps username → active player id for reconnect hints.
pub async fn store_session(redis_url: &str, username: &str, player_id: PlayerId) -> anyhow::Result<()> {
    #[cfg(feature = "redis")]
    {
        let client = redis::Client::open(redis_url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        let key = format!("session:{username}");
        let _: () = redis::cmd("SET")
            .arg(&key)
            .arg(player_id.0.to_string())
            .arg("EX")
            .arg(86400)
            .query_async(&mut conn)
            .await?;
        return Ok(());
    }
    #[cfg(not(feature = "redis"))]
    {
        let _ = (redis_url, username, player_id);
        Ok(())
    }
}

pub async fn clear_session(redis_url: &str, username: &str) -> anyhow::Result<()> {
    #[cfg(feature = "redis")]
    {
        let client = redis::Client::open(redis_url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;
        let key = format!("session:{username}");
        let _: () = redis::cmd("DEL").arg(&key).query_async(&mut conn).await?;
    }
    #[cfg(not(feature = "redis"))]
    {
        let _ = (redis_url, username);
    }
    Ok(())
}
