use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "data")]
pub enum Status {
    Created,
    Failed(FailOperation),
    Running,
    Killed,
    Finished,
    Timeout,
}

/// Where did the task fail
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "operation", content = "data")]
pub enum FailOperation {
    /// Failed to spawn child
    OnSpawn(String),
    /// Failed after timeout
    OnTimeout(OnTimeoutOrKillFailOperation),
    /// Failed after kill signal
    OnKill(OnTimeoutOrKillFailOperation),
    /// Failed during wait
    OnWait,
}

/// On timeout or kill signal we attempt to kill the child process and wait for it to finish.
/// So we can fail on kill or on wait.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "operation", content = "data")]
pub enum OnTimeoutOrKillFailOperation {
    /// Failed to kill child
    OnKill(String),
    /// Failed to wait for child
    OnWait(String),
}

pub struct Data {
    pub id: String,
    pub status: RwLock<Status>,
}

pub struct Handle {
    tx: mpsc::Sender<()>,
    termination_notify: Arc<Notify>,
    data: Arc<Data>,
}

impl Handle {
    pub async fn status(&self) -> Status {
        self.data.status.read().await.clone()
    }

    pub fn id(&self) -> &str {
        &self.data.id
    }

    #[tracing::instrument(name = "kill", skip(self), fields(id=self.id()))]
    pub async fn kill(&self) {
        match self.tx.send(()).await {
            Ok(_) => {
                tracing::info!("Sent kill signal. Waiting for termination");

                let _ = self.termination_notify.notified().await;
            }
            Err(_) => tracing::warn!("Failed to send kill signal"),
        }
    }
}

pub struct Task {
    rx: mpsc::Receiver<()>,
    termination_notify: Arc<Notify>,
    data: Arc<Data>,
}

impl Task {
    pub fn new(id: String) -> (Self, Handle) {
        let (tx, rx) = mpsc::channel(1);

        let termination_notify = Arc::new(Notify::new());

        let data = Arc::new(Data {
            id,
            status: RwLock::new(Status::Created),
        });

        let handle = Handle {
            tx,
            termination_notify: termination_notify.clone(),
            data: data.clone(),
        };

        let task = Self {
            rx,
            termination_notify,
            data,
        };

        (task, handle)
    }

    fn id(&self) -> &str {
        &self.data.id
    }

    async fn set_status(&self, status: Status) {
        *self.data.status.write().await = status
    }

    #[tracing::instrument(name = "status", skip_all)]
    async fn set_status_and_log(&self, status: Status) {
        tracing::debug!(?status, "Setting status");

        self.set_status(status).await;
    }

    #[tracing::instrument(name = "kill", skip_all)]
    async fn wait_for_kill_signal(&mut self) {
        if self.rx.recv().await.is_some() {
            tracing::info!("Received kill signal");

            return;
        }

        tracing::warn!("No more signals");
    }

    /// Returns `Err` if the task failed to spawn.
    /// Task status is always set accordingly.
    #[tracing::instrument(skip(self), fields(id=self.id()))]
    pub async fn run(mut self, timeout: Duration) -> Result<(), std::io::Error> {
        let mut child = match Command::new("cmd")
            .args(["/C", "timeout", "/T", "10", "/NOBREAK"])
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(?err, "Failed to spawn child");

                self.set_status_and_log(Status::Failed(FailOperation::OnSpawn(err.to_string())))
                    .await;

                return Err(err);
            }
        };

        self.set_status_and_log(Status::Running).await;

        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill child");
                    self.set_status_and_log(Status::Failed(FailOperation::OnTimeout(OnTimeoutOrKillFailOperation::OnKill(err.to_string())))).await;
                }

                if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for child");
                    self.set_status_and_log(Status::Failed(FailOperation::OnTimeout(OnTimeoutOrKillFailOperation::OnWait(err.to_string())))).await;
                }

                self.set_status_and_log(Status::Timeout).await;
            },
            _ = self.wait_for_kill_signal() => {

                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill child");
                    self.set_status_and_log(Status::Failed(FailOperation::OnKill(OnTimeoutOrKillFailOperation::OnKill(err.to_string())))).await;
                }

                if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for child");
                    self.set_status_and_log(Status::Failed(FailOperation::OnKill(OnTimeoutOrKillFailOperation::OnWait(err.to_string())))).await;
                }

                self.set_status_and_log(Status::Killed).await;
            },
            res = child.wait() => {
                match res {
                    Ok(status) => {
                        tracing::debug!(?status, "Child exited with status");
                        self.set_status_and_log(Status::Finished).await;
                    },
                    Err(err) => {
                        tracing::error!(?err, "Failed to wait for child");
                        self.set_status_and_log(Status::Failed(FailOperation::OnWait)).await;
                    }
                }
            }
        }

        tracing::debug!("Notifying of termination");
        self.termination_notify.notify_waiters();

        tracing::debug!("Terminated");

        Ok(())
    }
}
