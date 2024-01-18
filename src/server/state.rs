use super::task::{Handle, Status, Task};
use std::{
    collections::HashMap,
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
}

pub struct ApiStateInner {
    api_token: String,
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

        let timeout = std::time::Duration::from_secs(20);

        let (task, task_handle) = Task::new(id.clone());

        let mut task_handles = self.task_handles.write().await;
        task_handles.insert(id.clone(), task_handle);

        tokio::spawn(async move {
            let (tx, mut rx) = tokio::io::duplex(100);

            tokio::spawn(async move {
                let mut chunk = [0; 256];
                while let Ok(n) = rx.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }

                    let chunk = String::from_utf8_lossy(&chunk[..n]);
                    tracing::debug!(id=%task_id, "{chunk}");
                }

                tracing::debug!(id=%task_id, "Finished reading");
            });

            let _ = task
                .run::<_, tokio::fs::File>(timeout, Some(tx), None)
                .await;

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

    pub async fn kill_task(&self, id: &str) -> Option<Status> {
        let mut task_handles = self.task_handles.write().await;

        // match task_handles.remove(&id) {
        //     Some(task_handle) => {
        //         task_handle.kill().await;
        //         let status = task_handle.status().await;
        //         Some(status)
        //     }
        //     None => None,
        // }

        match task_handles.get_mut(id) {
            Some(task_handle) => {
                task_handle.kill().await;

                let status = task_handle.status().await;

                Some(status)
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
