//! Live `wss://` connectivity check against a real server.
//!
//! Ignored by default (needs network). Run against production with:
//!
//! ```text
//! cargo test -p openmmo-client --test wss_login -- --ignored --nocapture
//! ```
//!
//! This exercises the same `tokio-tungstenite` + rustls path the game client
//! uses. v0.1.0 shipped a client that panicked here because rustls had no
//! crypto provider, and no test covered TLS — the smoke test at the time used a
//! separate Python client, which brought its own TLS stack.

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEFAULT_URL: &str = "wss://server-production-2a39.up.railway.app/ws";

#[tokio::test]
#[ignore = "requires network access to a live server"]
async fn wss_connect_and_login() {
    // Mirrors `net::install_crypto_provider` (integration tests cannot import
    // from a binary crate).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let url = std::env::var("OPENMMO_SERVER_URL").unwrap_or_else(|_| DEFAULT_URL.to_string());
    assert!(url.starts_with("wss://"), "test needs a TLS url, got {url}");
    eprintln!("connecting to {url}");

    // The panic this guards against happened inside connect_async.
    let (mut ws, _) = connect_async(&url).await.expect("wss handshake");

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        % 1_000_000;
    let login = serde_json::json!({
        "type": "login",
        "username": format!("tlstest{suffix}"),
        "character_name": format!("TlsTest{suffix}"),
        "password": "integration-test",
    });
    ws.send(Message::Text(login.to_string()))
        .await
        .expect("send login");

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(!remaining.is_zero(), "timed out waiting for login_result");

        let Ok(Some(Ok(msg))) = tokio::time::timeout(remaining, ws.next()).await else {
            panic!("connection closed before login_result");
        };
        let Message::Text(text) = msg else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).expect("server sent json");
        if value["type"] == "login_result" {
            assert_eq!(value["success"], true, "login failed: {}", value["message"]);
            eprintln!("login_result ok: {}", value["message"]);
            return;
        }
    }
}
