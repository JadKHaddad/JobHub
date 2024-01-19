use super::ws::{ClientMessage, ServerMessage};
use axum::extract::ws::{Message, WebSocket};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::net::SocketAddr;
use tokio::sync::{broadcast, mpsc};

enum WSChannelInternalAction {
    Send(Message),
    Close,
}

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

    #[tracing::instrument(name = "websocket_incoming", skip_all, fields(addr = %addr))]
    async fn process_incoming(
        addr: SocketAddr,
        client_messages_sender: mpsc::Sender<ClientMessage>,
        internal_sender: mpsc::Sender<WSChannelInternalAction>,
        mut ws_receiver: SplitStream<WebSocket>,
    ) {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            tracing::trace!(?msg, "Message received");

            let msg = match msg {
                Message::Text(text) => text,
                Message::Binary(_) => {
                    tracing::warn!("Binary message received. Ignoring");
                    continue;
                }
                Message::Ping(_) => {
                    tracing::trace!("Ping message received. Responding with Pong");
                    let pong_response = Message::Pong(vec![]);

                    match internal_sender
                        .send(WSChannelInternalAction::Send(pong_response))
                        .await
                    {
                        Ok(_) => {}
                        Err(err) => {
                            tracing::error!(
                                ?err,
                                "Failed to send pong response. Sender was probably dropped"
                            );
                        }
                    }

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
                    tracing::warn!(?err, "Failed to parse message");
                    continue;
                }
            };

            match client_messages_sender.send(msg).await {
                Ok(_) => {}
                Err(err) => {
                    tracing::error!(?err, "Failed to forward message to state");
                    break;
                }
            }
        }

        match internal_sender.send(WSChannelInternalAction::Close).await {
            Ok(_) => {}
            Err(err) => {
                tracing::error!(
                    ?err,
                    "Failed to send close signal. Sender was probably dropped"
                );
            }
        }

        tracing::debug!("Receiver closed");
    }

    #[tracing::instrument(name = "websocket_outgoing", skip_all, fields(addr = %addr))]
    async fn process_outgoing(
        addr: SocketAddr,
        mut broadcast_receiver: broadcast::Receiver<ServerMessage>,
        mut internal_receiver: mpsc::Receiver<WSChannelInternalAction>,
        mut ws_sender: SplitSink<WebSocket, Message>,
    ) {
        loop {
            let msg = tokio::select! {
                msg = internal_receiver.recv() => {msg}
                msg = async {
                        if let Ok(msg) = broadcast_receiver.recv().await {
                            let msg = serde_json::to_string(&msg).unwrap_or_default();
                            let msg = Message::Text(msg);
                            let msg = WSChannelInternalAction::Send(msg);
                            return Some(msg)
                        }

                        None
                } => { msg }
            };

            if let Some(msg) = msg {
                match msg {
                    WSChannelInternalAction::Send(msg) => {
                        tracing::trace!(?msg, "Sending message");
                        match ws_sender.send(msg).await {
                            Ok(_) => {}
                            Err(err) => {
                                tracing::error!(?err, "Failed to send message");
                                break;
                            }
                        }
                    }

                    WSChannelInternalAction::Close => {
                        tracing::debug!("Closing connection");
                        break;
                    }
                }
            }
        }

        tracing::debug!("Sender closed");
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

        let (ws_sender, ws_receiver) = socket.split();
        let (internal_sender, internal_receiver) = mpsc::channel(1);
        let broadcast_receiver = self.broadcast_sender.subscribe();

        let mut recv_task = tokio::spawn(ConnectionManager::process_incoming(
            addr,
            client_messages_sender,
            internal_sender,
            ws_receiver,
        ));

        let mut send_task = tokio::spawn(ConnectionManager::process_outgoing(
            addr,
            broadcast_receiver,
            internal_receiver,
            ws_sender,
        ));

        tokio::select! {
            _ = (&mut send_task)  => {
                let _ = recv_task.await;
            },
            _ = (&mut recv_task) => {
                let _ = send_task.await;
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

impl Drop for ConnectionManager {
    fn drop(&mut self) {
        tracing::trace!("Connection manager dropped");
    }
}
