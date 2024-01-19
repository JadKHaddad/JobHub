use std::net::SocketAddr;

use super::ws::ServerMessage;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct ConnectionManager {
    pub broadcast_sender: broadcast::Sender<ServerMessage>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let (broadcast_sender, _) = broadcast::channel(100);

        Self { broadcast_sender }
    }

    pub fn broadcast(&self, msg: ServerMessage) {
        let _ = self.broadcast_sender.send(msg);
    }

    pub async fn accept_connection(self, socket: WebSocket, user_agent: String, addr: SocketAddr) {
        tracing::info!(?addr, %user_agent,  "Websocket connected");

        let (mut sender, mut receiver) = socket.split();
        let mut broadcast_receiver = self.broadcast_sender.subscribe();

        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                tracing::info!(?msg, ?addr, "Websocket message received");
            }

            tracing::debug!(?addr, "Websocket closed");
        });

        let mut send_task = tokio::spawn(async move {
            while let Ok(msg) = broadcast_receiver.recv().await {
                let msg = serde_json::to_string(&msg).unwrap_or_default();
                let msg = Message::Text(msg);

                tracing::debug!(?msg, ?addr, "Sending websocket message");

                match sender.send(msg).await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!(?err, ?addr, "Failed to send websocket message");
                        break;
                    }
                }
            }
        });

        tokio::select! {
            _ = (&mut send_task)  => {
                recv_task.abort();
            },
            _ = (&mut recv_task) => {
                send_task.abort();
            }
        }

        tracing::debug!(?addr, "Websocket context destroyed");
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
