use serde::Serialize;
use std::{process::ExitStatus, sync::Arc, time::Duration};
use tokio::{
    io::AsyncWrite,
    process::Command,
    sync::{
        mpsc::{self},
        RwLock,
    },
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "content")]
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
#[serde(tag = "exit_status", content = "content")]
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
    /// Used to send cancel signal to the task
    ///
    /// This is not a CancellationToken because dropping the handle should cancel the task
    tx: mpsc::Sender<()>,
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
    ///
    /// This will not wait for the task to finish. Waiting for the task to finish may cause a bad response times for the api.
    /// Running tasks will be locked until the task is finished, which may take a long time.
    /// Locking the tasks will prevent other tasks from running.
    #[tracing::instrument(name = "cancel_siganl", skip(self), fields(id=self.id()))]
    pub async fn send_cancel_signal(&self) {
        match self.tx.send(()).await {
            Ok(_) => {
                tracing::info!("Sent cancel signal");
            }
            Err(_) => tracing::warn!("Failed to send cancel signal. Taks was probably dropped"),
        }
    }
}

pub struct Task {
    rx: mpsc::Receiver<()>,
    data: Arc<Data>,
}

impl Task {
    pub fn new(id: String) -> (Self, Handle) {
        let (tx, rx) = mpsc::channel(1);

        let data = Arc::new(Data {
            id,
            status: RwLock::new(Status::Created),
        });

        let handle = Handle {
            tx,
            data: data.clone(),
        };

        let task = Self { rx, data };

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

    #[tracing::instrument(name = "cancel_siganl", skip_all)]
    async fn wait_for_cancel_signal(&mut self) {
        if self.rx.recv().await.is_some() {
            tracing::info!("Received cancel signal");

            return;
        }

        tracing::warn!("No more signals. Handle was probably dropped");
    }

    #[tracing::instrument(skip_all, fields(id=self.id(), timeout))]
    pub async fn run<O, E>(
        mut self,
        timeout: Duration,
        stdout_writer: Option<O>,
        stderr_writer: Option<E>,
    ) where
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
            Command::new("powershell")
                .args(["-File", "loop_numbers.ps1"])
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
        } else {
            Command::new("bash")
                .args(["-c", "while true; do echo 1; sleep 1; done"])
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
        };

        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(?err, "Failed to spawn OS process");

                self.set_status_and_log(Status::Failed {
                    operation: FailOperation::OnSpawn,
                })
                .await;

                return;
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

                match child.kill().await {
                    Ok(_) => {
                        tracing::debug!("Killed OS process");

                        match child.wait().await {
                            Ok(exit_status) => {
                                tracing::debug!(?exit_status, "OS process exited with status");
                                Status::Timeout
                            },
                            Err(err) => {
                                tracing::error!(?err, "Failed to wait for OS process");
                                Status::Failed{ operation: FailOperation::AfterTimeoutOnWait }
                            }
                        }
                    },

                    Err(err) => {
                        tracing::error!(?err, "Failed to kill OS process");
                        Status::Failed{ operation: FailOperation::AfterTimeoutOnKill }
                    }
                }


            },
            _ = self.wait_for_cancel_signal() => {

                match child.kill().await {
                    Ok(_) => {
                        tracing::debug!("Killed OS process");

                        match child.wait().await {
                            Ok(_) => {
                                Status::Canceled
                            },
                            Err(err) => {
                                tracing::error!(?err, "Failed to wait for OS process");
                                Status::Failed{ operation: FailOperation::AfterCancelOnWait }
                            }
                        }
                    },

                    Err(err) => {
                        tracing::error!(?err, "Failed to kill OS process");
                        Status::Failed{ operation: FailOperation::AfterCancelOnKill }
                    }
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

        tracing::debug!("Terminated");
    }
}
