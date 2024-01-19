use std::net::SocketAddr;

use super::ws::{ClientMessage, ServerMessage};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tokio::sync::{broadcast, mpsc};

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

    #[tracing::instrument(name = "websocket", skip_all, fields(addr = %addr))]
    pub async fn accept_connection(
        &self,
        client_messages_sender: mpsc::Sender<ClientMessage>,
        socket: WebSocket,
        user_agent: String,
        addr: SocketAddr,
    ) {
        tracing::info!(%user_agent, "Connected");

        let (mut sender, mut receiver) = socket.split();
        let mut broadcast_receiver = self.broadcast_sender.subscribe();

        let mut recv_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = receiver.next().await {
                tracing::trace!(?msg, ?addr, "Websocket message received");

                let msg = match msg {
                    Message::Text(text) => text,
                    Message::Binary(_) => {
                        tracing::warn!("Binary message received. Ignoring");
                        continue;
                    }
                    Message::Ping(_) => {
                        tracing::warn!("Ping message received. Ignoring");
                        continue;
                    }
                    Message::Pong(_) => {
                        tracing::warn!("Pong message received. Ignoring");
                        continue;
                    }
                    Message::Close(_) => {
                        tracing::warn!("Close message received. Ignoring");
                        continue;
                    }
                };

                let msg = match serde_json::from_str::<ClientMessage>(&msg) {
                    Ok(msg) => msg,
                    Err(err) => {
                        tracing::warn!(?err, ?addr, "Failed to parse websocket message");
                        continue;
                    }
                };

                match client_messages_sender.send(msg).await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!(?err, ?addr, "Failed forward websocket message to state");
                        break;
                    }
                }
            }

            tracing::debug!(?addr, "Websocket closed");
        });

        let mut send_task = tokio::spawn(async move {
            while let Ok(msg) = broadcast_receiver.recv().await {
                let msg = serde_json::to_string(&msg).unwrap_or_default();
                let msg = Message::Text(msg);

                tracing::trace!(?msg, ?addr, "Sending websocket message");

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

        tracing::debug!("Context destroyed");
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
