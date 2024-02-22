use serde::Serialize;
use std::{io::Read, sync::Arc, time::Duration};
use tokio::sync::{
    mpsc::{self},
    RwLock,
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "content")]
pub enum Status {
    Created,
    Failed { reason: String },
    Running,
    Canceled,
    Exited,
    Timeout,
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
    pub async fn run_download_and_unzip_from_google_drive_link(
        mut self,
        timeout: Duration,
        download_url: url::Url,
        project_dir: std::path::PathBuf,
    ) {
        self.set_status_and_log(Status::Running).await;

        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                Status::Timeout
            },
            _ = self.wait_for_cancel_signal() => {

                Status::Canceled
            },
            status = async move {
                let response = reqwest::get(download_url).await;

                match response {
                    Ok(response) => {
                        if !response.status().is_success() {
                            tracing::error!("Failed to download file");

                            return Status::Failed {
                                reason: format!("Failed to download file: {}", response.status())
                            };
                        }
                        match response.bytes().await {
                            Ok(bytes) => {
                                tracing::debug!("Downloaded file");
                                match zip::ZipArchive::new(std::io::Cursor::new(bytes)) {
                                    Ok(mut zip) => {
                                        for i in 0..zip.len() {
                                            let Ok(file)= zip.by_index(i) else {
                                                tracing::error!("Failed to get file");

                                                return Status::Failed {
                                                    reason: "Failed to get file".to_string()
                                                };
                                            };
                                            let file_name = project_dir.join(file.name());

                                            let Ok(bytes) = file.bytes().collect::<Result<Vec<u8>, _>>() else {
                                                tracing::error!("Failed to read file");

                                                return Status::Failed {
                                                    reason: "Failed to read file".to_string()
                                                };
                                            };

                                            match file_name.parent() {
                                                Some(parent) => {
                                                    if let Err(err) = tokio::fs::create_dir_all(parent).await {
                                                        tracing::error!(?err, "Failed to create parent directory");

                                                        return Status::Failed {
                                                            reason: format!("Failed to create parent directory: {err}")
                                                        };
                                                    }
                                                },
                                                None => {
                                                    tracing::error!("Failed to get parent directory");

                                                    return Status::Failed {
                                                        reason: "Failed to get parent directory".to_string()
                                                    };
                                                }
                                            }

                                            match tokio::fs::File::create(&file_name).await {
                                                Ok(mut outfile) => {
                                                    if let Err(err) = tokio::io::copy(&mut std::io::Cursor::new(bytes), &mut outfile).await {
                                                        tracing::error!(?err,?file_name, "Failed to copy file");

                                                        return Status::Failed {
                                                            reason: format!("Failed to copy file: {err}")
                                                        };
                                                    }

                                                    tracing::debug!(?file_name, "Unzipped file");
                                                },
                                                Err(err) => {
                                                    tracing::error!(?err, "Failed to create file");

                                                    return Status::Failed {
                                                        reason: format!("Failed to create file: {err}")
                                                    };
                                                }
                                            }
                                        };
                                        Status::Exited
                                    },
                                    Err(err) => {
                                        tracing::error!(?err, "Failed to open file");

                                        Status::Failed {
                                            reason: format!("Failed to open file: {err}")
                                        }
                                    }
                                }
                            },
                            Err(err) => {
                                tracing::error!(?err, "Failed to download file");

                                Status::Failed {
                                    reason: format!("Failed to download file: {err}")
                                }
                            }
                        }
                    },
                    Err(err) => {
                        tracing::error!(?err, "Failed to download file");

                        Status::Failed {
                            reason: format!("Failed to download file: {err}")
                        }
                    }
                }
            } => {
                status
            },
        };

        self.set_status_and_log(status).await;

        tracing::debug!("Terminated");
    }
}
