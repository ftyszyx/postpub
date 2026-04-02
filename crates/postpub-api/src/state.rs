use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use postpub_core::AppContext;
use postpub_types::{
    GenerateArticleRequest, GenerationEvent, GenerationTaskStatus, GenerationTaskSummary,
};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub context: Arc<AppContext>,
    pub generation_manager: GenerationManager,
}

impl ApiState {
    pub fn new(context: Arc<AppContext>) -> Self {
        Self {
            context,
            generation_manager: GenerationManager::default(),
        }
    }
}

#[derive(Clone, Default)]
pub struct GenerationManager {
    tasks: Arc<RwLock<HashMap<String, GenerationTaskSummary>>>,
    streams: Arc<RwLock<HashMap<String, broadcast::Sender<GenerationEvent>>>>,
}

impl GenerationManager {
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

        let (sender, _) = broadcast::channel(64);
        self.tasks.write().await.insert(id.clone(), summary.clone());
        self.streams
            .write()
            .await
            .insert(id.clone(), sender.clone());

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
                    "generation task accepted",
                    GenerationTaskStatus::Running,
                )
                .await;
            manager
                .push_event(
                    &id,
                    "retrieval",
                    "collecting source material",
                    GenerationTaskStatus::Running,
                )
                .await;

            let result = context.generation_service().generate(request).await;
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
                            "compose",
                            "generation output created",
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

        summary
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
        self.push_event(
            id,
            "done",
            "generation completed",
            GenerationTaskStatus::Succeeded,
        )
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

        if let Some(sender) = self.streams.read().await.get(id) {
            let _ = sender.send(event);
        }
    }
}
