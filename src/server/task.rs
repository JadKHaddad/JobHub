use serde::Serialize;
use std::{ffi::OsStr, process::ExitStatus, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    process::Command,
    sync::{mpsc, RwLock},
};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "type", content = "content")]
pub enum Status {
    Download(DownloadZipFileStatus),
    Process(ProcessStatus),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "content")]
pub enum DownloadZipFileStatus {
    Created,
    Failed { reason: String },
    Running,
    Canceled,
    Exited,
    Timeout,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "status", content = "content")]
pub enum ProcessStatus {
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

impl From<ExitStatus> for ProcessStatus {
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
            Err(_) => tracing::warn!("Failed to send cancel signal. Task was probably dropped"),
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
            status: RwLock::new(Status::Process(ProcessStatus::Created)),
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

    async fn copy_io<R, W>(reader: &mut R, writter: &mut W)
    where
        R: AsyncRead + Unpin + ?Sized,
        W: AsyncWrite + Unpin + ?Sized,
    {
        if let Err(err) = tokio::io::copy(reader, writter).await {
            tracing::error!(?err, "Failed to copy to writer");
        }

        tracing::debug!("Finished copying to writer");
    }

    #[tracing::instrument(skip_all, fields(id=task_id))]
    async fn copy_stdout<R, W>(task_id: String, reader: &mut R, writter: &mut W)
    where
        R: AsyncRead + Unpin + ?Sized,
        W: AsyncWrite + Unpin + ?Sized,
    {
        Self::copy_io(reader, writter).await;
    }

    #[tracing::instrument(skip_all, fields(id=task_id))]
    async fn copy_stderr<R, W>(task_id: String, reader: &mut R, writter: &mut W)
    where
        R: AsyncRead + Unpin + ?Sized,
        W: AsyncWrite + Unpin + ?Sized,
    {
        Self::copy_io(reader, writter).await;
    }

    #[tracing::instrument(skip_all, fields(id=self.id(), timeout))]
    pub async fn run_os_process<S, I, O, E>(
        mut self,
        command: S,
        args: I,
        timeout: Duration,
        stdout_writer: Option<O>,
        stderr_writer: Option<E>,
    ) where
        S: AsRef<OsStr>,
        I: IntoIterator<Item = S>,
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

        let child = Command::new(command)
            .args(args)
            .stdout(stdout)
            .stderr(stderr)
            .spawn();

        let mut child = match child {
            Ok(child) => child,
            Err(err) => {
                tracing::error!(?err, "Failed to spawn OS process");

                self.set_status_and_log(Status::Process(ProcessStatus::Failed {
                    operation: FailOperation::OnSpawn,
                }))
                .await;

                return;
            }
        };

        if let Some(mut write) = stdout_writer {
            let id = self.id().to_string();
            let stdout = child.stdout.take();
            tokio::spawn(async move {
                if let Some(mut stdout) = stdout {
                    Self::copy_stdout(id, &mut stdout, &mut write).await;
                }
            });
        }

        if let Some(mut write) = stderr_writer {
            let id = self.id().to_string();
            let stderr = child.stderr.take();
            tokio::spawn(async move {
                if let Some(mut stderr) = stderr {
                    Self::copy_stderr(id, &mut stderr, &mut write).await;
                }
            });
        }

        self.set_status_and_log(Status::Process(ProcessStatus::Running))
            .await;

        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                match child.kill().await {
                    Ok(_) => {
                        tracing::debug!("Killed OS process");

                        match child.wait().await {
                            Ok(exit_status) => {
                                tracing::debug!(?exit_status, "OS process exited with status");
                                ProcessStatus::Timeout
                            },
                            Err(err) => {
                                tracing::error!(?err, "Failed to wait for OS process");
                                ProcessStatus::Failed{ operation: FailOperation::AfterTimeoutOnWait }
                            }
                        }
                    },

                    Err(err) => {
                        tracing::error!(?err, "Failed to kill OS process");
                        ProcessStatus::Failed{ operation: FailOperation::AfterTimeoutOnKill }
                    }
                }
            },
            _ = self.wait_for_cancel_signal() => {

                match child.kill().await {
                    Ok(_) => {
                        tracing::debug!("Killed OS process");

                        match child.wait().await {
                            Ok(_) => {
                                ProcessStatus::Canceled
                            },
                            Err(err) => {
                                tracing::error!(?err, "Failed to wait for OS process");
                                ProcessStatus::Failed{ operation: FailOperation::AfterCancelOnWait }
                            }
                        }
                    },

                    Err(err) => {
                        tracing::error!(?err, "Failed to kill OS process");
                        ProcessStatus::Failed{ operation: FailOperation::AfterCancelOnKill }
                    }
                }
            },
            res = child.wait() => {
                match res {
                    Ok(exit_status) => {
                        tracing::debug!(?exit_status, "OS process exited with status");
                        ProcessStatus::from(exit_status)
                    },
                    Err(err) => {
                        tracing::error!(?err, "Failed to wait for OS process");
                        ProcessStatus::Failed{ operation: FailOperation::OnWait }
                    }
                }
            }
        };

        self.set_status_and_log(Status::Process(status)).await;

        tracing::debug!("Terminated");
    }

    #[tracing::instrument(skip_all, fields(id=self.id(), timeout))]
    pub async fn run_download_and_unzip_from_download_url(
        mut self,
        timeout: Duration,
        download_url: url::Url,
        project_dir: std::path::PathBuf,
    ) {
        self.set_status_and_log(Status::Download(DownloadZipFileStatus::Running))
            .await;

        let status = tokio::select! {
            _ = tokio::time::sleep(timeout) => {
                tracing::debug!("Timeout");

                DownloadZipFileStatus::Timeout
            },
            _ = self.wait_for_cancel_signal() => {

                DownloadZipFileStatus::Canceled
            },
            result = Self::download_and_unzip_from_download_url(download_url, project_dir) => {
                match result {
                    Ok(_) => {
                        DownloadZipFileStatus::Exited
                    },
                    Err(err) => {
                        DownloadZipFileStatus::Failed { reason: err.to_string() }
                    }
                }
            },
        };

        self.set_status_and_log(Status::Download(status)).await;

        tracing::debug!("Terminated");
    }

    async fn download_and_unzip_from_download_url(
        download_url: url::Url,
        project_dir: std::path::PathBuf,
    ) -> Result<(), DownloadError> {
        let response = reqwest::get(download_url)
            .await
            .map_err(DownloadError::Reqwest)?;

        let bytes = response.bytes().await.map_err(DownloadError::Bytes)?;
        tracing::debug!("Zip file downloaded");

        let zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(DownloadError::Zip)?;

        tracing::debug!("Unzipping files");

        // ZipFile is not Send -> spawn_blocking
        tokio::task::spawn_blocking(move || Self::unzip(zip, project_dir))
            .await
            .map_err(|_| DownloadError::BlockingTask)?
    }

    fn unzip(
        mut zip: zip::ZipArchive<std::io::Cursor<axum::body::Bytes>>,
        project_dir: std::path::PathBuf,
    ) -> Result<(), DownloadError> {
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(DownloadError::Zip)?;
            let file_name = std::path::PathBuf::from(file.name());

            // Strip all directories
            let file_name = file_name
                .file_name()
                .ok_or(DownloadError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid file name",
                )))?;

            let file_name = project_dir.join(file_name);

            let mut outfile = std::fs::File::create(&file_name).map_err(DownloadError::Io)?;

            let _ = std::io::copy(&mut file, &mut outfile).map_err(DownloadError::Io)?;

            tracing::debug!(?file_name, "Unzipped file");
        }

        Ok(())
    }
}

/// Inner error type for [`Task::download_and_unzip_from_download_url`]
#[derive(Debug, thiserror::Error)]
enum DownloadError {
    #[error("Reqwest error: {0}")]
    Reqwest(reqwest::Error),
    #[error("Failed to extract bytes: {0}")]
    Bytes(reqwest::Error),
    #[error("Zip error: {0}")]
    Zip(zip::result::ZipError),
    #[error("Io error: {0}")]
    Io(std::io::Error),
    #[error("Failed to spawn blocking task")]
    BlockingTask,
}
