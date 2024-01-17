use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio::sync::RwLock;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub enum Status {
    Created,
    Running,
    Killed,
    Finished,
    Timeout,
}

#[derive(Clone)]
pub struct Data {
    pub id: String,
    pub status: Arc<RwLock<Status>>,
}

pub struct Handle {
    tx: mpsc::Sender<()>,
    termination_notify: Arc<Notify>,
    data: Data,
}

impl Handle {
    pub async fn status(&self) -> Status {
        *self.data.status.read().await
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
    data: Data,
}

impl Task {
    pub fn new(id: String) -> (Self, Handle) {
        let (tx, rx) = mpsc::channel(1);

        let termination_notify = Arc::new(Notify::new());

        let data = Data {
            id,
            status: Arc::new(RwLock::new(Status::Created)),
        };

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

    #[tracing::instrument(skip(self), fields(id=self.id()))]
    pub async fn run(mut self, timeout: Duration) -> Result<(), std::io::Error> {
        let mut child = Command::new("cmd")
            .args(["/C", "timeout", "/T", "10", "/NOBREAK"])
            .spawn()?;

        self.set_status_and_log(Status::Running).await;

        tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                child.kill().await?;
                child.wait().await?;

                self.set_status_and_log(Status::Timeout).await;
            },
            _ = self.wait_for_kill_signal() => {

                child.kill().await?;
                child.wait().await?;

                self.set_status_and_log(Status::Killed).await;
            },
            _ = child.wait() => {
                tracing::debug!("Child exited");

                self.set_status_and_log(Status::Finished).await;
            }
        }

        tracing::debug!("Notifying of termination");
        self.termination_notify.notify_waiters();

        tracing::debug!("Terminated");

        Ok(())
    }
}
