use super::task::{Handle, Status, Task};
use std::{
    collections::HashMap,
    ops::Deref,
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::RwLock,
};

/// I want my [`ApiState`] to be [`Clone`] and [`Send`] and [`Sync`] as is.
/// So I'm wrapping [`ApiState::inner`] in an [`Arc`].
#[derive(Clone)]
pub struct ApiState {
    inner: Arc<ApiStateInner>,
}

impl ApiState {
    pub fn new(api_token: String, projects_dir: String) -> Self {
        Self {
            inner: Arc::new(ApiStateInner::new(api_token, projects_dir)),
        }
    }

    pub fn api_token_valid(&self, api_token: &str) -> bool {
        api_token == self.api_token
    }
}

/// Collecting relevant data for a task.
struct TaskData {
    chat_id: String,
    handle: Handle,
}

pub struct ApiStateInner {
    api_token: String,
    /// Contains all the tasks that are currently running.
    /// The key is the task id.
    tasks: Arc<RwLock<HashMap<String, TaskData>>>,
    /// I'm not wrapping [`ApiStateInner`] in a lock.
    /// So it's a good old [`AtomicU32`].
    current_id: AtomicU32,
    projects_dir: String,
}

impl ApiStateInner {
    pub fn new(api_token: String, projects_dir: String) -> Self {
        Self {
            api_token,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            current_id: AtomicU32::new(0),
            projects_dir,
        }
    }

    pub fn generate_random_chat_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn increment_current_task_id(&self) -> u32 {
        let id = self.current_id.load(Ordering::Relaxed);

        self.current_id.store(id + 1, Ordering::Relaxed);

        id
    }

    fn project_dir(&self, project_name: &str) -> PathBuf {
        PathBuf::from(&self.projects_dir).join(project_name)
    }

    pub async fn run_download_task(
        &self,
        chat_id: String,
        download_url: url::Url,
        project_name: String,
    ) -> Result<String, std::io::Error> {
        // Let's create a directory for the project
        let project_dir = self.project_dir(&project_name);
        tokio::fs::create_dir_all(&project_dir).await?;

        let id = self.increment_current_task_id().to_string();
        let task_id = id.clone();

        let timeout = std::time::Duration::from_secs(600);

        let (task, task_handle) = Task::new(id.clone());
        let task_data = TaskData {
            chat_id,
            handle: task_handle,
        };

        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task_data);

        let tasks = self.tasks.clone();

        tokio::spawn(async move {
            task.run_download_and_unzip_from_download_url(timeout, download_url, project_dir)
                .await;

            // TODO: remove after adding a database.
            // Keeping task in memory for 15 minutes after it's done.
            // simulating an in-memory database.

            tracing::debug!(id=%task_id, "Task finished. Waiting 15 minutes before removing it from memory");
            tokio::time::sleep(std::time::Duration::from_secs(900)).await;
            tracing::debug!(id=%task_id, "Removing task from memory");
            let mut tasks = tasks.write().await;
            tasks.remove(&task_id);
        });

        Ok(id)
    }

    #[tracing::instrument(skip_all, fields(id=task_id))]
    async fn trace_stdout<R: AsyncRead + Unpin>(task_id: String, stdout_rx: R) {
        let buf_reader = BufReader::new(stdout_rx);
        let mut lines = buf_reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            tracing::trace!("{line}");
        }

        tracing::debug!("Finished reading stdout");
    }

    #[tracing::instrument(skip_all, fields(id=task_id))]
    async fn trace_stderr<R: AsyncRead + Unpin>(task_id: String, stderr_rx: R) {
        let buf_reader = BufReader::new(stderr_rx);
        let mut lines = buf_reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            tracing::error!("{line}");
        }

        tracing::debug!("Finished reading stderr");
    }

    pub async fn run_gs_log_to_locust_converter_task(
        &self,
        chat_id: String,
        project_name: String,
    ) -> Result<String, GsLogToLocustConverterError> {
        let project_dir = self.project_dir(&project_name);

        if !project_dir.exists() {
            return Err(GsLogToLocustConverterError::NotFound);
        }

        let id = self.increment_current_task_id().to_string();
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

        let task_data = TaskData {
            chat_id,
            handle: task_handle,
        };

        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), task_data);

        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            let (stdout_tx, stdout_rx) = tokio::io::duplex(100);
            let (stderr_tx, stderr_rx) = tokio::io::duplex(100);

            let stdout_task_id = task_id.clone();
            let stderr_task_id = task_id.clone();

            tokio::spawn(async move {
                Self::trace_stdout(stdout_task_id, stdout_rx).await;
            });

            tokio::spawn(async move {
                Self::trace_stderr(stderr_task_id, stderr_rx).await;
            });

            let command = cfg!(target_os = "windows")
                .then(|| "python")
                .unwrap_or("python3")
                .to_string();

            let path_to_gs_log_to_locust_converter_script = PathBuf::from("ML_ETL")
                .join("GS")
                .join("Logfiles")
                .join("GSLogToLocustConverter.py")
                .to_string_lossy()
                .to_string();

            let project_dir = project_dir.to_string_lossy().to_string();

            let args = vec![
                path_to_gs_log_to_locust_converter_script,
                String::from("--directory"),
                project_dir,
                String::from("--force"),
            ];

            task.run_os_process(command, args, timeout, Some(stdout_tx), Some(stderr_tx))
                .await;

            // TODO: remove after adding a database.
            // Keeping task in memory for 15 minutes after it's done.
            // simulating an in-memory database.

            tracing::debug!(id=%task_id, "Task finished. Waiting 15 minutes before removing it from memory");
            tokio::time::sleep(std::time::Duration::from_secs(900)).await;
            tracing::debug!(id=%task_id, "Removing task from memory");
            let mut tasks = tasks.write().await;
            tasks.remove(&task_id);
        });

        Ok(id)
    }

    /// Send a cancel signal to the task with the given id and return immediately.
    /// The Terminated task will be removed fom memory in a different tokio task which is spawned by [`ApiStateInner::run_task`].
    pub async fn cancel_task<'a>(&self, id: &'a str, chat_id: &str) -> Option<&'a str> {
        let tasks = self.tasks.read().await;
        match tasks.get(id) {
            Some(task_data) if task_data.chat_id == chat_id => {
                task_data.handle.send_cancel_signal().await;

                Some(id)
            }
            _ => None,
        }
    }

    pub async fn task_status(&self, id: &str, chat_id: &str) -> Option<Status> {
        let tasks = self.tasks.read().await;
        match tasks.get(id) {
            Some(task_data) if task_data.chat_id == chat_id => {
                let status = task_data.handle.status().await;

                Some(status)
            }
            _ => None,
        }
    }

    pub async fn list_files(&self, project_name: String) -> Result<Vec<String>, ListFilesError> {
        let project_dir = PathBuf::from(&self.projects_dir).join(project_name);

        if !project_dir.exists() {
            return Err(ListFilesError::NotFound);
        }

        let mut read_dir = tokio::fs::read_dir(project_dir).await?;

        let mut files: Vec<String> = Vec::new();

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy().to_string();

            files.push(file_name);
        }

        Ok(files)
    }

    pub async fn get_file(
        &self,
        project_name: String,
        file_name: String,
    ) -> Result<String, GetFileError> {
        let project_dir = PathBuf::from(&self.projects_dir).join(project_name);

        if !project_dir.exists() {
            return Err(GetFileError::NotFound);
        }

        let file_path = project_dir.join(file_name);

        if !file_path.exists() {
            return Err(GetFileError::NotFound);
        }

        let file_content = tokio::fs::read_to_string(file_path).await?;

        Ok(file_content)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GsLogToLocustConverterError {
    #[error("Project not found")]
    NotFound,
}

#[derive(Debug, thiserror::Error)]
pub enum ListFilesError {
    #[error("Project not found")]
    NotFound,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GetFileError {
    #[error("Project/File not found")]
    NotFound,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Deref for ApiState {
    type Target = ApiStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Drop for ApiStateInner {
    fn drop(&mut self) {
        tracing::trace!("Api state inner dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::task::{ProcessStatus, Status::Process};

    fn init_tracing() {
        if std::env::var_os("RUST_LOG").is_none() {
            std::env::set_var("RUST_LOG", "job_hub=trace");
        }

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    // cargo test --package job_hub --lib -- server::state::tests::run_gs_log_to_locust_converter_task --exact --nocapture --ignored
    // python .\ML_ETL\GS\Logfiles\GSLogToLocustConverter.py --directory .\projects\project\ --force
    // python3 ML_ETL/GS/Logfiles/GSLogToLocustConverter.py --directory projects/project --force
    #[tokio::test]
    #[ignore = "Observation test"]
    async fn run_gs_log_to_locust_converter_task() {
        init_tracing();

        let api_state = ApiState::new("".to_string(), "projects".to_string());

        let chat_id = "chat_id".to_string();
        let project_name = "project".to_string();

        let task_id = api_state
            .run_gs_log_to_locust_converter_task(chat_id.clone(), project_name)
            .await
            .expect("Failed to start task");

        loop {
            match api_state.task_status(&task_id, &chat_id).await {
                Some(Process(ProcessStatus::Created)) => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                Some(status) => {
                    tracing::info!(status = ?status, "Task status");
                    break;
                }
                _ => break,
            }
        }
    }
}
