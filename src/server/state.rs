use crate::server::ws::{IoType, ServerMessage, TaskIoChunk};

use super::{
    connection_manager::ConnectionManager,
    task::{Handle, Status, Task},
    ws::ClientMessage,
};
use axum::extract::ws::WebSocket;
use std::{
    collections::HashMap,
    net::SocketAddr,
    ops::Deref,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::{io::AsyncReadExt, sync::RwLock};

/// I want my [`ApiState`] to be [`Clone`] and [`Send`] and [`Sync`] as is.
/// So I'm wrapping [`ApiState::inner`] in an [`Arc`].
#[derive(Clone)]
pub struct ApiState {
    inner: Arc<ApiStateInner>,
}

impl ApiState {
    pub fn new(api_token: String) -> Self {
        Self {
            inner: Arc::new(ApiStateInner::new(api_token)),
        }
    }

    pub fn api_token_valid(&self, api_token: &str) -> bool {
        api_token == self.api_token
    }

    pub async fn accept_connection(self, socket: WebSocket, user_agent: String, addr: SocketAddr) {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ClientMessage>(100);

        self.inner
            .connection_manager
            .accept_connection(tx, socket, user_agent, addr)
            .await;

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                // Deal with the message
                tracing::info!(?msg, "Received message from client");
            }
        });
    }
}

pub struct ApiStateInner {
    api_token: String,
    connection_manager: ConnectionManager,
    /// Contains all the tasks that are currently running.
    /// The key is the task id.
    /// The value is the [`Handle`] of the task ._.
    task_handles: Arc<RwLock<HashMap<String, Handle>>>,
    /// I'm not wrapping [`ApiStateInner`] in a lock.
    /// So it's a good old [`AtomicU32`].
    current_id: AtomicU32,
}

impl ApiStateInner {
    pub fn new(api_token: String) -> Self {
        Self {
            api_token,
            connection_manager: ConnectionManager::new(),
            task_handles: Arc::new(RwLock::new(HashMap::new())),
            current_id: AtomicU32::new(0),
        }
    }

    fn increment_current_id(&self) -> u32 {
        let id = self.current_id.load(Ordering::Relaxed);

        self.current_id.store(id + 1, Ordering::Relaxed);

        id
    }

    pub async fn run_task(&self) -> String {
        let id = self.increment_current_id().to_string();
        let task_id = id.clone();

        let timeout = std::time::Duration::from_secs(600);

        let (task, task_handle) = Task::new(id.clone());

        // TODO: Move to tests
        // {
        // Test canceling the task before running it. and expect it to be canceled immediately after running.
        // task_handle.cancel().await;

        // Test dropping the handle before running the task. and expect it to be canceled immediately after running.
        // drop(task_handle);
        // }

        let mut task_handles = self.task_handles.write().await;
        task_handles.insert(id.clone(), task_handle);

        let connection_manager = self.connection_manager.clone();
        tokio::spawn(async move {
            let (stdout_tx, mut stdout_rx) = tokio::io::duplex(100);
            let (stderr_tx, mut stderr_rx) = tokio::io::duplex(100);

            let stdout_task_id = task_id.clone();
            let stderr_task_id = task_id;

            let stdout_connection_manager = connection_manager.clone();
            let stderr_connection_manager = connection_manager;

            // While forwarding the outputs we can save the chunks to the database or send them to a client.
            tokio::spawn(async move {
                let mut chunk = [0; 256];
                while let Ok(n) = stdout_rx.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }

                    let chunk = String::from_utf8_lossy(&chunk[..n]);
                    tracing::debug!(id=%stdout_task_id, "{chunk}");

                    let msg = ServerMessage::TaskIoChunk(TaskIoChunk {
                        id: stdout_task_id.clone(),
                        chunk: chunk.to_string(),
                        io_type: IoType::Stdout,
                    });

                    stdout_connection_manager.broadcast(msg);
                }

                tracing::debug!(id=%stdout_task_id, "Finished reading stdout");
            });

            tokio::spawn(async move {
                let mut chunk = [0; 256];
                while let Ok(n) = stderr_rx.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }

                    let chunk = String::from_utf8_lossy(&chunk[..n]);
                    tracing::error!(id=%stderr_task_id, "{chunk}");

                    let msg = ServerMessage::TaskIoChunk(TaskIoChunk {
                        id: stderr_task_id.clone(),
                        chunk: chunk.to_string(),
                        io_type: IoType::Stderr,
                    });

                    stderr_connection_manager.broadcast(msg);
                }

                tracing::debug!(id=%stderr_task_id, "Finished reading stderr");
            });

            let _ = task.run(timeout, Some(stdout_tx), Some(stderr_tx)).await;

            // let (tx, mut rx) = tokio::sync::mpsc::channel::<TaskOutputFrame>(100);
            // let io_line_mapper = TaskOutputFrameMapper { tx, task_id };

            // tokio::spawn(async move {
            //     while let Some(task_output_frame) = rx.recv().await {
            //         let task_output_frame =
            //             String::from_utf8_lossy(&task_output_frame.output_frame);
            //         tracing::info!(?task_output_frame);
            //     }
            // });

            // let file = tokio::fs::File::create("out.txt").await.unwrap();

            // let mut task_handles = task_handles.write().await;
            // task_handles.remove(&id);
        });

        id
    }

    /// Send a cancel signal to the task with the given id and return immediately.
    pub async fn cancel_task<'a>(&self, id: &'a str) -> Option<&'a str> {
        // let mut task_handles = self.task_handles.write().await;

        // match task_handles.remove(&id) {
        //     Some(task_handle) => {
        //         task_handle.kill().await;
        //         let status = task_handle.status().await;
        //         Some(status)
        //     }
        //     None => None,
        // }

        let task_handles = self.task_handles.read().await;
        match task_handles.get(id) {
            Some(task_handle) => {
                task_handle.send_cancel_signal().await;

                Some(id)
            }
            None => None,
        }
    }

    pub async fn task_status(&self, id: &str) -> Option<Status> {
        let task_handles = self.task_handles.read().await;

        match task_handles.get(id) {
            Some(task_handle) => {
                let status = task_handle.status().await;

                Some(status)
            }
            None => None,
        }
    }
}

impl Deref for ApiState {
    type Target = ApiStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
