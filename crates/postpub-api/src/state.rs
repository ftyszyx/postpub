use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::Utc;
use postpub_core::{AppContext, GenerationProgressReporter, PostpubError, PublishProgressReporter};
use postpub_types::{
    GenerateArticleRequest, GenerationEvent, GenerationTaskStatus, GenerationTaskSummary,
    PublishArticleRequest, PublishEvent, PublishOutput, PublishTaskStatus, PublishTaskSummary,
};
use tokio::sync::{broadcast, mpsc, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub context: Arc<AppContext>,
    pub generation_manager: GenerationManager,
    pub publish_manager: PublishManager,
}

impl ApiState {
    pub fn new(context: Arc<AppContext>) -> Self {
        let generation_tasks_file = context.paths().generation_tasks_file();
        let publish_tasks_file = context.paths().publish_tasks_file();
        Self {
            context,
            generation_manager: GenerationManager::new(generation_tasks_file),
            publish_manager: PublishManager::new(publish_tasks_file),
        }
    }
}

#[derive(Clone)]
pub struct GenerationManager {
    tasks: Arc<RwLock<HashMap<String, GenerationTaskSummary>>>,
    streams: Arc<RwLock<HashMap<String, broadcast::Sender<GenerationEvent>>>>,
    storage_path: Arc<PathBuf>,
}

impl GenerationManager {
    pub fn new(storage_path: PathBuf) -> Self {
        let restored = load_persisted_tasks(&storage_path);
        Self {
            tasks: Arc::new(RwLock::new(restored)),
            streams: Arc::new(RwLock::new(HashMap::new())),
            storage_path: Arc::new(storage_path),
        }
    }

    pub async fn create_task(
        &self,
        context: Arc<AppContext>,
        request: GenerateArticleRequest,
    ) -> GenerationTaskSummary {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let summary = GenerationTaskSummary {
            id: id.clone(),
            request: request.clone(),
            status: GenerationTaskStatus::Pending,
            created_at: now,
            updated_at: now,
            events: vec![],
            output: None,
            error: None,
        };

        self.tasks.write().await.insert(id.clone(), summary.clone());
        self.ensure_stream(&id).await;
        self.persist_tasks().await;
        self.spawn_task(context, id.clone(), request);

        summary
    }

    pub async fn retry_task(
        &self,
        context: Arc<AppContext>,
        id: &str,
    ) -> postpub_core::Result<GenerationTaskSummary> {
        let request = {
            let mut tasks = self.tasks.write().await;
            let Some(task) = tasks.get_mut(id) else {
                return Err(PostpubError::NotFound(format!(
                    "generation task not found: {id}"
                )));
            };

            if matches!(
                task.status,
                GenerationTaskStatus::Pending | GenerationTaskStatus::Running
            ) {
                return Err(PostpubError::Conflict(format!(
                    "generation task is already running: {id}"
                )));
            }

            task.status = GenerationTaskStatus::Pending;
            task.updated_at = Utc::now();
            task.output = None;
            task.error = None;
            task.request.clone()
        };

        self.ensure_stream(id).await;
        self.persist_tasks().await;
        self.push_event(
            id,
            "bootstrap",
            "收到重试请求，正在重新执行任务",
            GenerationTaskStatus::Pending,
        )
        .await;

        self.spawn_task(context, id.to_string(), request);

        self.get_task(id).await.ok_or_else(|| {
            PostpubError::NotFound(format!("generation task not found after retry: {id}"))
        })
    }

    fn spawn_task(&self, context: Arc<AppContext>, id: String, request: GenerateArticleRequest) {
        let manager = self.clone();
        tracing::info!(
            task_id = %id,
            topic = %request.topic,
            reference_url_count = request.reference_urls.len(),
            save_output = request.save_output,
            "generation task started"
        );
        tokio::spawn(async move {
            manager
                .push_event(
                    &id,
                    "bootstrap",
                    "任务已进入执行队列",
                    GenerationTaskStatus::Running,
                )
                .await;

            let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<(String, String)>();
            let progress_manager = manager.clone();
            let progress_task_id = id.clone();
            let progress_forwarder = tokio::spawn(async move {
                while let Some((stage, message)) = progress_rx.recv().await {
                    progress_manager
                        .push_event(
                            &progress_task_id,
                            &stage,
                            &message,
                            GenerationTaskStatus::Running,
                        )
                        .await;
                }
            });

            let reporter = GenerationProgressReporter::new(move |stage, message| {
                let _ = progress_tx.send((stage, message));
            });
            let task_reporter = reporter.clone();

            let result = context
                .generation_service()
                .generate_with_progress(request, task_reporter)
                .await;
            drop(reporter);
            let _ = progress_forwarder.await;
            match result {
                Ok(output) => {
                    tracing::info!(
                        task_id = %id,
                        mode = %output.mode,
                        source_count = output.sources.len(),
                        "generation task completed"
                    );
                    manager
                        .push_event(
                            &id,
                            "finalize",
                            "生成结果已创建",
                            GenerationTaskStatus::Running,
                        )
                        .await;
                    manager.finish_success(&id, output).await;
                }
                Err(error) => {
                    tracing::error!(
                        task_id = %id,
                        error = %error,
                        "generation task failed"
                    );
                    manager.finish_failure(&id, error.to_string()).await;
                }
            }
        });
    }

    async fn ensure_stream(&self, id: &str) -> broadcast::Sender<GenerationEvent> {
        let mut streams = self.streams.write().await;
        if let Some(sender) = streams.get(id) {
            return sender.clone();
        }

        let (sender, _) = broadcast::channel(64);
        streams.insert(id.to_string(), sender.clone());
        sender
    }

    pub async fn list_tasks(&self) -> Vec<GenerationTaskSummary> {
        let mut tasks = self
            .tasks
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        tasks
    }

    pub async fn get_task(&self, id: &str) -> Option<GenerationTaskSummary> {
        self.tasks.read().await.get(id).cloned()
    }

    pub async fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<GenerationEvent>> {
        self.streams
            .read()
            .await
            .get(id)
            .map(|stream| stream.subscribe())
    }

    async fn finish_success(&self, id: &str, output: postpub_types::GenerationOutput) {
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = GenerationTaskStatus::Succeeded;
                task.updated_at = Utc::now();
                task.output = Some(output);
                task.error = None;
            }
        }
        self.persist_tasks().await;
        self.push_event(id, "done", "生成完成", GenerationTaskStatus::Succeeded)
            .await;
    }

    async fn finish_failure(&self, id: &str, error: String) {
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = GenerationTaskStatus::Failed;
                task.updated_at = Utc::now();
                task.error = Some(error.clone());
            }
        }
        self.persist_tasks().await;
        self.push_event(id, "failed", &error, GenerationTaskStatus::Failed)
            .await;
    }

    pub async fn push_event(
        &self,
        id: &str,
        stage: &str,
        message: &str,
        status: GenerationTaskStatus,
    ) {
        let event = GenerationEvent {
            task_id: id.to_string(),
            timestamp: Utc::now(),
            stage: stage.to_string(),
            message: message.to_string(),
            status: status.clone(),
        };

        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = status;
                task.updated_at = event.timestamp;
                task.events.push(event.clone());
            }
        }
        self.persist_tasks().await;

        if let Some(sender) = self.streams.read().await.get(id) {
            let _ = sender.send(event);
        }
    }

    async fn persist_tasks(&self) {
        let snapshot = self
            .tasks
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();

        if let Some(parent) = self.storage_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                tracing::error!(
                    path = %parent.display(),
                    error = %error,
                    "failed to create generation task storage directory"
                );
                return;
            }
        }

        match serde_json::to_string_pretty(&snapshot) {
            Ok(content) => {
                if let Err(error) = fs::write(self.storage_path.as_ref(), content) {
                    tracing::error!(
                        path = %self.storage_path.display(),
                        error = %error,
                        "failed to persist generation tasks"
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    path = %self.storage_path.display(),
                    error = %error,
                    "failed to serialize generation tasks"
                );
            }
        }
    }
}

#[derive(Clone)]
pub struct PublishManager {
    tasks: Arc<RwLock<HashMap<String, PublishTaskSummary>>>,
    streams: Arc<RwLock<HashMap<String, broadcast::Sender<PublishEvent>>>>,
    storage_path: Arc<PathBuf>,
}

impl PublishManager {
    pub fn new(storage_path: PathBuf) -> Self {
        let restored = load_persisted_publish_tasks(&storage_path);
        Self {
            tasks: Arc::new(RwLock::new(restored)),
            streams: Arc::new(RwLock::new(HashMap::new())),
            storage_path: Arc::new(storage_path),
        }
    }

    pub async fn create_task(
        &self,
        context: Arc<AppContext>,
        request: PublishArticleRequest,
    ) -> PublishTaskSummary {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let summary = PublishTaskSummary {
            id: id.clone(),
            request: request.clone(),
            status: PublishTaskStatus::Pending,
            created_at: now,
            updated_at: now,
            events: vec![],
            output: None,
            error: None,
        };

        self.tasks.write().await.insert(id.clone(), summary.clone());
        self.ensure_stream(&id).await;
        self.persist_tasks().await;
        self.spawn_task(context, id.clone(), request);

        summary
    }

    pub async fn retry_task(
        &self,
        context: Arc<AppContext>,
        id: &str,
    ) -> postpub_core::Result<PublishTaskSummary> {
        let request = {
            let mut tasks = self.tasks.write().await;
            let Some(task) = tasks.get_mut(id) else {
                return Err(PostpubError::NotFound(format!(
                    "publish task not found: {id}"
                )));
            };

            if matches!(
                task.status,
                PublishTaskStatus::Pending | PublishTaskStatus::Running
            ) {
                return Err(PostpubError::Conflict(format!(
                    "publish task is already running: {id}"
                )));
            }

            task.status = PublishTaskStatus::Pending;
            task.updated_at = Utc::now();
            task.output = None;
            task.error = None;
            task.request.clone()
        };

        self.ensure_stream(id).await;
        self.persist_tasks().await;
        self.push_event(
            id,
            "bootstrap",
            "收到重试请求，正在重新执行发布任务",
            PublishTaskStatus::Pending,
        )
        .await;

        self.spawn_task(context, id.to_string(), request);

        self.get_task(id).await.ok_or_else(|| {
            PostpubError::NotFound(format!("publish task not found after retry: {id}"))
        })
    }

    fn spawn_task(&self, context: Arc<AppContext>, id: String, request: PublishArticleRequest) {
        let manager = self.clone();
        tracing::info!(
            task_id = %id,
            article_relative_path = %request.article_relative_path,
            target_id = %request.target_id,
            mode = %request.mode,
            "publish task started"
        );
        tokio::spawn(async move {
            manager
                .push_event(
                    &id,
                    "bootstrap",
                    "发布任务已进入执行队列",
                    PublishTaskStatus::Running,
                )
                .await;

            let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<(String, String)>();
            let progress_manager = manager.clone();
            let progress_task_id = id.clone();
            let progress_forwarder = tokio::spawn(async move {
                while let Some((stage, message)) = progress_rx.recv().await {
                    progress_manager
                        .push_event(
                            &progress_task_id,
                            &stage,
                            &message,
                            PublishTaskStatus::Running,
                        )
                        .await;
                }
            });

            let reporter = PublishProgressReporter::new(move |stage, message| {
                let _ = progress_tx.send((stage, message));
            });
            let task_reporter = reporter.clone();

            let result = context
                .publish_service()
                .publish_with_progress(request, task_reporter)
                .await;
            drop(reporter);
            let _ = progress_forwarder.await;

            match result {
                Ok(output) => {
                    tracing::info!(
                        task_id = %id,
                        target_id = %output.target_id,
                        platform_type = %output.platform_type,
                        "publish task completed"
                    );
                    manager
                        .push_event(
                            &id,
                            "finalize",
                            "发布结果已创建",
                            PublishTaskStatus::Running,
                        )
                        .await;
                    manager.finish_success(&id, output).await;
                }
                Err(error) => {
                    tracing::error!(
                        task_id = %id,
                        error = %error,
                        "publish task failed"
                    );
                    manager.finish_failure(&id, error.to_string()).await;
                }
            }
        });
    }

    async fn ensure_stream(&self, id: &str) -> broadcast::Sender<PublishEvent> {
        let mut streams = self.streams.write().await;
        if let Some(sender) = streams.get(id) {
            return sender.clone();
        }

        let (sender, _) = broadcast::channel(64);
        streams.insert(id.to_string(), sender.clone());
        sender
    }

    pub async fn list_tasks(&self) -> Vec<PublishTaskSummary> {
        let mut tasks = self
            .tasks
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        tasks
    }

    pub async fn get_task(&self, id: &str) -> Option<PublishTaskSummary> {
        self.tasks.read().await.get(id).cloned()
    }

    pub async fn subscribe(&self, id: &str) -> Option<broadcast::Receiver<PublishEvent>> {
        self.streams
            .read()
            .await
            .get(id)
            .map(|stream| stream.subscribe())
    }

    async fn finish_success(&self, id: &str, output: PublishOutput) {
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = PublishTaskStatus::Succeeded;
                task.updated_at = Utc::now();
                task.output = Some(output);
                task.error = None;
            }
        }
        self.persist_tasks().await;
        self.push_event(id, "done", "发布完成", PublishTaskStatus::Succeeded)
            .await;
    }

    async fn finish_failure(&self, id: &str, error: String) {
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = PublishTaskStatus::Failed;
                task.updated_at = Utc::now();
                task.error = Some(error.clone());
            }
        }
        self.persist_tasks().await;
        self.push_event(id, "failed", &error, PublishTaskStatus::Failed)
            .await;
    }

    pub async fn push_event(
        &self,
        id: &str,
        stage: &str,
        message: &str,
        status: PublishTaskStatus,
    ) {
        let event = PublishEvent {
            task_id: id.to_string(),
            timestamp: Utc::now(),
            stage: stage.to_string(),
            message: message.to_string(),
            status: status.clone(),
        };

        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(id) {
                task.status = status;
                task.updated_at = event.timestamp;
                task.events.push(event.clone());
            }
        }
        self.persist_tasks().await;

        if let Some(sender) = self.streams.read().await.get(id) {
            let _ = sender.send(event);
        }
    }

    async fn persist_tasks(&self) {
        let snapshot = self
            .tasks
            .read()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();

        if let Some(parent) = self.storage_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                tracing::error!(
                    path = %parent.display(),
                    error = %error,
                    "failed to create publish task storage directory"
                );
                return;
            }
        }

        match serde_json::to_string_pretty(&snapshot) {
            Ok(content) => {
                if let Err(error) = fs::write(self.storage_path.as_ref(), content) {
                    tracing::error!(
                        path = %self.storage_path.display(),
                        error = %error,
                        "failed to persist publish tasks"
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    path = %self.storage_path.display(),
                    error = %error,
                    "failed to serialize publish tasks"
                );
            }
        }
    }
}

fn load_persisted_tasks(path: &Path) -> HashMap<String, GenerationTaskSummary> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            tracing::error!(
                path = %path.display(),
                error = %error,
                "failed to read persisted generation tasks"
            );
            return HashMap::new();
        }
    };

    let tasks = match serde_json::from_str::<Vec<GenerationTaskSummary>>(&content) {
        Ok(tasks) => tasks,
        Err(error) => {
            tracing::error!(
                path = %path.display(),
                error = %error,
                "failed to parse persisted generation tasks"
            );
            return HashMap::new();
        }
    };

    tasks
        .into_iter()
        .map(normalize_restored_task)
        .map(|task| (task.id.clone(), task))
        .collect()
}

fn normalize_restored_task(mut task: GenerationTaskSummary) -> GenerationTaskSummary {
    if matches!(
        task.status,
        GenerationTaskStatus::Pending | GenerationTaskStatus::Running
    ) {
        let message = "服务重启导致任务中断".to_string();
        let timestamp = Utc::now();
        task.status = GenerationTaskStatus::Failed;
        task.updated_at = timestamp;
        task.error = Some(message.clone());
        task.events.push(GenerationEvent {
            task_id: task.id.clone(),
            timestamp,
            stage: "failed".to_string(),
            message,
            status: GenerationTaskStatus::Failed,
        });
    }

    task
}

fn load_persisted_publish_tasks(path: &Path) -> HashMap<String, PublishTaskSummary> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HashMap::new(),
        Err(error) => {
            tracing::error!(
                path = %path.display(),
                error = %error,
                "failed to read persisted publish tasks"
            );
            return HashMap::new();
        }
    };

    let tasks = match serde_json::from_str::<Vec<PublishTaskSummary>>(&content) {
        Ok(tasks) => tasks,
        Err(error) => {
            tracing::error!(
                path = %path.display(),
                error = %error,
                "failed to parse persisted publish tasks"
            );
            return HashMap::new();
        }
    };

    tasks
        .into_iter()
        .map(normalize_restored_publish_task)
        .map(|task| (task.id.clone(), task))
        .collect()
}

fn normalize_restored_publish_task(mut task: PublishTaskSummary) -> PublishTaskSummary {
    if matches!(
        task.status,
        PublishTaskStatus::Pending | PublishTaskStatus::Running
    ) {
        let message = "服务重启导致发布任务中断".to_string();
        let timestamp = Utc::now();
        task.status = PublishTaskStatus::Failed;
        task.updated_at = timestamp;
        task.error = Some(message.clone());
        task.events.push(PublishEvent {
            task_id: task.id.clone(),
            timestamp,
            stage: "failed".to_string(),
            message,
            status: PublishTaskStatus::Failed,
        });
    }

    task
}
