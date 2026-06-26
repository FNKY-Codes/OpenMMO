use futures_util::{SinkExt, StreamExt};
use openmmo_engine::NetCommand;
use openmmo_protocol::{decode_server, encode_client, ClientMessage, ServerMessage};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

pub async fn network_thread(
    mut cmd_rx: std::sync::mpsc::Receiver<NetCommand>,
    msg_tx: std::sync::mpsc::Sender<ServerMessage>,
) {
    let mut ws_sink = None;
    let mut ws_stream = None;

    loop {
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                NetCommand::Connect {
                    url,
                    username,
                    character,
                } => {
                    match connect_async(&url).await {
                        Ok((ws, _)) => {
                            info!("Connected to {url}");
                            let (sink, stream) = ws.split();
                            ws_sink = Some(sink);
                            ws_stream = Some(stream);
                            let login = ClientMessage::Login {
                                username,
                                character_name: character,
                            };
                            if let Some(s) = &mut ws_sink {
                                let _ = s
                                    .send(Message::Text(encode_client(&login).unwrap().into()))
                                    .await;
                            }
                        }
                        Err(e) => {
                            error!("Connect failed: {e}");
                            let _ = msg_tx.send(ServerMessage::Error {
                                message: format!("Connection failed: {e}"),
                            });
                        }
                    }
                }
                NetCommand::Send(msg) => {
                    if let Some(s) = &mut ws_sink {
                        if let Ok(json) = encode_client(&msg) {
                            let _ = s.send(Message::Text(json.into())).await;
                        }
                    }
                }
            }
        }

        if let Some(stream) = &mut ws_stream {
            while let Ok(Some(Ok(msg))) = tokio::time::timeout(
                std::time::Duration::from_millis(10),
                stream.next(),
            )
            .await
            {
                if let Message::Text(text) = msg {
                    if let Ok(server_msg) = decode_server(&text) {
                        let _ = msg_tx.send(server_msg);
                    }
                }
            }
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
}
