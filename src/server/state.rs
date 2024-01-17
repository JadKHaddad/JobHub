use super::task::{Handle, Status, Task};
use std::{
    collections::HashMap,
    ops::Deref,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;

/// I want my [`ServerState`] to be [`Clone`] and [`Send`] and [`Sync`] as is.
/// So I'm wrapping [`ServerState::inner`] in an [`Arc`].
#[derive(Clone)]
pub struct ServerState {
    inner: Arc<ServerStateInner>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ServerStateInner::new()),
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ServerStateInner {
    /// Contains all the tasks that are currently running.
    /// The key is the task id.
    /// The value is the [`Handle`] of the task ._.
    task_handles: Arc<RwLock<HashMap<String, Handle>>>,
    /// I'm not wrapping [`ServerStateInner`] in a lock.
    /// So it's a good old [`AtomicU32`].
    current_id: AtomicU32,
}

impl ServerStateInner {
    pub fn new() -> Self {
        Self {
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

        let timeout = std::time::Duration::from_secs(20);

        let (task, task_handle) = Task::new(id.clone());

        let mut task_handles = self.task_handles.write().await;
        task_handles.insert(id.clone(), task_handle);

        // let task_handles = self.task_handles.clone();
        tokio::spawn(async move {
            let _ = task.run(timeout).await;
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

impl Default for ServerStateInner {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for ServerState {
    type Target = ServerStateInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
