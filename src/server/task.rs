use serde::Serialize;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::AsyncWrite,
    process::Command,
    sync::{
        mpsc::{self, error::TrySendError},
        Notify, RwLock,
    },
};
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
    OnSpawn,
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
pub enum OnTimeoutOrKillFailOperation {
    /// Failed to kill child
    OnKill,
    /// Failed to wait for child
    OnWait,
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
            std::process::Stdio::inherit()
        };

        let stderr = if stderr_writer.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::inherit()
        };

        #[cfg(target_os = "windows")]
        let child = Command::new("cmd")
            .args(["/C", "timeout", "/T", "10", "/NOBREAK"])
            .stdout(stdout)
            .stderr(stderr)
            .spawn();

        #[cfg(not(target_os = "windows"))]
        let child = Command::new("sleep")
            .args(["10"])
            .stdout(stdout)
            .stderr(stderr)
            .spawn();

        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(?err, "Failed to spawn child");

                self.set_status_and_log(Status::Failed(FailOperation::OnSpawn))
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
                }
            });
        }

        self.set_status_and_log(Status::Running).await;

        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill child");
                    Status::Failed(FailOperation::OnTimeout(OnTimeoutOrKillFailOperation::OnKill))
                } else if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for child");
                    Status::Failed(FailOperation::OnTimeout(OnTimeoutOrKillFailOperation::OnWait))
                } else {
                    Status::Timeout
                }
            },
            _ = self.wait_for_kill_signal() => {
                if let Err(err) = child.kill().await {
                    tracing::error!(?err, "Failed to kill child");
                    Status::Failed(FailOperation::OnKill(OnTimeoutOrKillFailOperation::OnKill))
                } else if let Err(err) = child.wait().await {
                    tracing::error!(?err, "Failed to wait for child");
                    Status::Failed(FailOperation::OnKill(OnTimeoutOrKillFailOperation::OnWait))
                } else {
                    Status::Killed
                }
            },
            res = child.wait() => {
                match res {
                    Ok(exit_status) => {
                        tracing::debug!(?exit_status, "Child exited with status");
                        Status::Finished
                    },
                    Err(err) => {
                        tracing::error!(?err, "Failed to wait for child");
                        Status::Failed(FailOperation::OnWait)
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

// dev

#[derive(Debug)]
pub struct TaskOutputFrame {
    pub task_id: String,
    pub output_frame: Vec<u8>,
}

pub struct TaskOutputFrameMapper {
    pub task_id: String,
    pub tx: mpsc::Sender<TaskOutputFrame>,
}

impl AsyncWrite for TaskOutputFrameMapper {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let output_frame = buf.to_vec();

        let task_output = TaskOutputFrame {
            task_id: self.task_id.clone(),
            output_frame,
        };

        match self.tx.try_send(task_output) {
            Ok(_) => Poll::Ready(Ok(buf.len())),
            Err(err) => match err {
                TrySendError::Full(_) => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                TrySendError::Closed(_) => {
                    Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, err)))
                }
            },
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
