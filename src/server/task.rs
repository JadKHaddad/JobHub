use serde::Serialize;
use std::{process::ExitStatus, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWrite,
    process::Command,
    sync::{
        mpsc::{self},
        Notify, RwLock,
    },
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "data")]
pub enum Status {
    Created,
    Failed { operation: FailOperation },
    Running,
    Canceled,
    Exited { exit_status: ExitedStatus },
    Timeout,
}

/// Where did the task fail
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "operation", content = "data")]
pub enum FailOperation {
    /// Failed to spawn OS process
    OnSpawn,
    /// Failed after timeout while killing OS process
    AfterTimeoutOnKill,
    /// Failed after timeout while waiting for OS process
    AfterTimeoutOnWait,
    /// Failed after cancel while killing OS process
    AfterCancelOnKill,
    /// Failed after cancel while waiting for OS process
    AfterCancelOnWait,
    /// Failed during wait
    OnWait,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "data")]
pub enum ExitedStatus {
    /// Exited with success
    Success,
    /// Exited with failure
    Failure { code: Option<i32> },
}

impl From<ExitStatus> for ExitedStatus {
    fn from(exit_status: ExitStatus) -> Self {
        if exit_status.success() {
            return Self::Success;
        }

        let code = exit_status.code();
        Self::Failure { code }
    }
}

impl From<ExitStatus> for Status {
    fn from(exit_status: ExitStatus) -> Self {
        Self::Exited {
            exit_status: exit_status.into(),
        }
    }
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

    /// If called before running the task, the task will be canceled immediately after spawning.
    // Problem: we should be able to cancel a task before it even starts. because it might take a long time to start or we might recieve a very qiuick cancel signal from client.
    // We also want this function to wait for the task to finish. So we can get the right status of the task after canceling it.
    // FIXME: find a way to wait for termination signal without &mut self.
    #[tracing::instrument(name = "cancel", skip(self), fields(id=self.id()))]
    pub async fn cancel(&self) {
        match self.tx.send(()).await {
            Ok(_) => {
                tracing::info!("Sent cancel signal. Waiting for termination");

                // if this code is called before the task is spawned we deadlock here.
                // let _ = self.termination_notify.notified().await;
            }
            Err(_) => tracing::warn!("Failed to send cancel signal"),
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

    #[tracing::instrument(name = "cancel", skip_all)]
    async fn wait_for_cancel_signal(&mut self) {
        if self.rx.recv().await.is_some() {
            tracing::info!("Received cancel signal");

            return;
        }

        tracing::warn!("No more signals");
    }

    // FIXME: Return something taht runs with wait method.
    // When run is called it should retun this thing that runs if the child spawned successfully.
    // Spawn erros will be handled by the caller and a fail response will be sent synchronously.
    // If it returns with waitable, the caller will send a success response and wait for the task to finish asynchronously.
    /// Returns `Err` if the task failed to spawn.
    /// Task status is always set accordingly.
    #[tracing::instrument(skip_all, fields(id=self.id(), timeout))]
    pub async fn run<O, E>(
        mut self,
        timeout: Duration,
        stdout_writer: Option<O>,
        stderr_writer: Option<E>,
    ) -> Result<(), std::io::Error>
    where
        O: 'static + AsyncWrite + Unpin + Send,
        E: 'static + AsyncWrite + Unpin + Send,
    {
        let stdout = if stdout_writer.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        };

        let stderr = if stderr_writer.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        };

        let child = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", "timeout", "/T", "10", "/NOBREAK"])
                .stdout(stdout)
                .stderr(stderr)
                .stdin(std::process::Stdio::null())
                .spawn()
        } else {
            Command::new("sleep")
                .args(["10"])
                .stdout(stdout)
                .stderr(stderr)
                .stdin(std::process::Stdio::null())
                .spawn()
        };

        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(?err, "Failed to spawn child");

                self.set_status_and_log(Status::Failed {
                    operation: FailOperation::OnSpawn,
                })
                .await;

                return Err(err);
            }
        };

        if let Some(mut write) = stdout_writer {
            let stdout = child.stdout.take();
            tokio::spawn(async move {
                if let Some(mut stdout) = stdout {
                    if let Err(err) = tokio::io::copy(&mut stdout, &mut write).await {
                        tracing::error!(?err, "Failed to copy stdout to writer");
                    }

                    tracing::debug!("Finished copying stdout to writer");
                }
            });
        }

        if let Some(mut write) = stderr_writer {
            let stderr = child.stderr.take();
            tokio::spawn(async move {
                if let Some(mut stderr) = stderr {
                    if let Err(err) = tokio::io::copy(&mut stderr, &mut write).await {
                        tracing::error!(?err, "Failed to copy stderr to writer");
                    }

                    tracing::debug!("Finished copying stderr to writer");
                }
            });
        }

        self.set_status_and_log(Status::Running).await;

        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill OS process");
                    Status::Failed{
                        operation: FailOperation::AfterTimeoutOnKill
                    }
                } else if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for OS process");
                    Status::Failed{
                        operation: FailOperation::AfterTimeoutOnWait
                    }
                } else {
                    Status::Timeout
                }
            },
            _ = self.wait_for_cancel_signal() => {
                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill OS process");
                    Status::Failed { operation: FailOperation::AfterCancelOnKill }
                } else if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for OS process");
                    Status::Failed{ operation: FailOperation::AfterCancelOnWait }
                } else {
                    Status::Canceled
                }
            },
            res = child.wait() => {
                match res {
                    Ok(exit_status) => {
                        tracing::debug!(?exit_status, "OS process exited with status");
                        Status::from(exit_status)
                    },
                    Err(err) => {
                        tracing::error!(?err, "Failed to wait for OS process");
                        Status::Failed{ operation: FailOperation::OnWait }
                    }
                }
            }
        };

        self.set_status_and_log(status).await;

        tracing::debug!("Notifying of termination");
        self.termination_notify.notify_waiters();

        tracing::debug!("Terminated");

        Ok(())
    }
}
