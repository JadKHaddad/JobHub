use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "message", content = "content")]
pub enum WSMessage {
    /// A message from the client to the server
    Client(ClientMessage),
    /// A message from the server to the client
    Server(ServerMessage),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "server_message", content = "content")]
pub enum ServerMessage {
    /// A Chunk of IO output from a task
    TaskIoChunk(TaskIoChunk),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskIoChunk {
    pub id: String,
    pub chunk: String,
    pub io_type: IoType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IoType {
    Stdout,
    Stderr,
}
