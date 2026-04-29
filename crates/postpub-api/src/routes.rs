use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post, put},
    Json, Router,
};
use futures::StreamExt;
use postpub_types::{
    ApiResponse, AppPathsInfo, ArticleDesign, BrowserEnvironmentStatus, ConfigBundle,
    CreateTemplateCategoryRequest, CreateTemplateRequest, GenerateArticleRequest, HealthStatus,
    MoveTemplateRequest, PublishArticleRequest, PublishTargetLoginStatus,
    RenameTemplateCategoryRequest, RenameTemplateRequest, TemplateCategorySummary,
    TemplateDocument, TemplateSummary, UiConfig, UpdateArticleContentRequest,
    UpdateTemplateContentRequest,
};
use tokio_stream::wrappers::BroadcastStream;

use crate::{error::ApiError, state::ApiState};

#[derive(Debug, serde::Deserialize)]
pub struct TemplateListQuery {
    category: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct BrowserStatusQuery {
    target_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct DeleteTasksRequest {
    ids: Vec<String>,
}

pub fn api_router() -> Router<ApiState> {
    Router::new()
        .route("/api/system/health", get(health))
        .route("/api/system/paths", get(paths))
        .route("/api/system/browser", get(browser_status))
        .route(
            "/api/system/browser/open/{*target_id}",
            post(open_browser_homepage),
        )
        .route(
            "/api/system/browser/profiles/{*target_id}",
            axum::routing::delete(clear_browser_profile),
        )
        .route("/api/config", get(get_config).put(save_config))
        .route("/api/config/default", get(get_default_config))
        .route("/api/config/ui", get(get_ui_config).put(save_ui_config))
        .route(
            "/api/templates/categories",
            get(list_template_categories).post(create_template_category),
        )
        .route(
            "/api/templates/categories/{category_name}",
            put(rename_template_category).delete(delete_template_category),
        )
        .route("/api/templates", get(list_templates).post(create_template))
        .route("/api/templates/actions/rename", post(rename_template))
        .route("/api/templates/actions/copy", post(copy_template))
        .route("/api/templates/actions/move", post(move_template))
        .route(
            "/api/templates/{*relative_path}",
            get(get_template)
                .put(update_template)
                .delete(delete_template),
        )
        .route("/api/articles", get(list_articles))
        .route(
            "/api/articles/design/{*relative_path}",
            get(get_article_design).put(save_article_design),
        )
        .route(
            "/api/articles/{*relative_path}",
            get(get_article).put(update_article).delete(delete_article),
        )
        .route(
            "/api/generation/tasks",
            get(list_generation_tasks).post(create_generation_task),
        )
        .route(
            "/api/generation/tasks/actions/delete",
            post(delete_generation_tasks),
        )
        .route(
            "/api/generation/tasks/{task_id}",
            get(get_generation_task).delete(delete_generation_task),
        )
        .route(
            "/api/generation/tasks/{task_id}/retry",
            post(retry_generation_task),
        )
        .route(
            "/api/generation/tasks/{task_id}/events",
            get(generation_events),
        )
        .route(
            "/api/publish/tasks",
            get(list_publish_tasks).post(create_publish_task),
        )
        .route(
            "/api/publish/tasks/actions/delete",
            post(delete_publish_tasks),
        )
        .route(
            "/api/publish/targets/{target_id}/login-status",
            post(check_publish_target_login_status),
        )
        .route(
            "/api/publish/tasks/{task_id}",
            get(get_publish_task).delete(delete_publish_task),
        )
        .route(
            "/api/publish/tasks/{task_id}/retry",
            post(retry_publish_task),
        )
        .route(
            "/api/publish/tasks/{task_id}/cancel",
            post(cancel_publish_task),
        )
        .route("/api/publish/tasks/{task_id}/events", get(publish_events))
}

async fn health(State(state): State<ApiState>) -> Json<ApiResponse<HealthStatus>> {
    Json(ApiResponse::ok(state.context.health_status()))
}

async fn paths(State(state): State<ApiState>) -> Json<ApiResponse<AppPathsInfo>> {
    Json(ApiResponse::ok(state.context.paths().as_info()))
}

async fn browser_status(
    State(state): State<ApiState>,
    Query(query): Query<BrowserStatusQuery>,
) -> Result<Json<ApiResponse<BrowserEnvironmentStatus>>, ApiError> {
    let status = state
        .context
        .browser_manager()
        .status(query.target_id.as_deref())
        .await?;
    Ok(Json(ApiResponse::ok(status)))
}

async fn clear_browser_profile(
    State(state): State<ApiState>,
    Path(target_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let profile_dir = state.context.browser_manager().clear_profile(&target_id)?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({
            "target_id": target_id,
            "profile_dir": profile_dir.display().to_string()
        }),
        "browser profile cleared",
    )))
}

async fn open_browser_homepage(
    State(state): State<ApiState>,
    Path(target_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let bundle = state.context.config_store().load_bundle()?;
    let target = bundle
        .config
        .publish_targets
        .into_iter()
        .find(|target| target.id == target_id)
        .ok_or_else(|| ApiError::not_found(format!("publish target not found: {target_id}")))?;

    let profile_dir = state
        .context
        .browser_manager()
        .open_target_homepage(&target)
        .await?;

    Ok(Json(ApiResponse::with_message(
        serde_json::json!({
            "target_id": target.id,
            "profile_dir": profile_dir.display().to_string(),
            "url": target.publish_url
        }),
        "browser homepage opened",
    )))
}

async fn get_config(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<ConfigBundle>>, ApiError> {
    let bundle = state.context.config_store().load_bundle()?;
    Ok(Json(ApiResponse::ok(bundle)))
}

async fn save_config(
    State(state): State<ApiState>,
    Json(bundle): Json<ConfigBundle>,
) -> Result<Json<ApiResponse<ConfigBundle>>, ApiError> {
    let store = state.context.config_store();
    store.save_config(&bundle.config)?;
    store.save_aiforge_config(&bundle.aiforge_config)?;
    store.save_ui_config(&bundle.ui_config)?;
    let saved = store.load_bundle()?;
    Ok(Json(ApiResponse::with_message(
        saved,
        "configuration saved",
    )))
}

async fn get_default_config() -> Json<ApiResponse<ConfigBundle>> {
    Json(ApiResponse::ok(ConfigBundle {
        config: postpub_types::PostpubConfig::default(),
        aiforge_config: postpub_types::AiforgeConfig::default(),
        ui_config: UiConfig::default(),
    }))
}

async fn get_ui_config(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<UiConfig>>, ApiError> {
    let ui_config = state.context.config_store().load_ui_config()?;
    Ok(Json(ApiResponse::ok(ui_config)))
}

async fn save_ui_config(
    State(state): State<ApiState>,
    Json(ui_config): Json<UiConfig>,
) -> Result<Json<ApiResponse<UiConfig>>, ApiError> {
    state.context.config_store().save_ui_config(&ui_config)?;
    let saved = state.context.config_store().load_ui_config()?;
    Ok(Json(ApiResponse::with_message(saved, "ui config saved")))
}

async fn list_template_categories(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<TemplateCategorySummary>>>, ApiError> {
    let categories = state.context.template_store().list_categories()?;
    Ok(Json(ApiResponse::ok(categories)))
}

async fn create_template_category(
    State(state): State<ApiState>,
    Json(request): Json<CreateTemplateCategoryRequest>,
) -> Result<Json<ApiResponse<TemplateCategorySummary>>, ApiError> {
    let category = state.context.template_store().create_category(&request)?;
    Ok(Json(ApiResponse::with_message(
        category,
        "template category created",
    )))
}

async fn rename_template_category(
    State(state): State<ApiState>,
    Path(category_name): Path<String>,
    Json(request): Json<RenameTemplateCategoryRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state
        .context
        .template_store()
        .rename_category(&category_name, &request)?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "category_name": category_name, "new_name": request.new_name }),
        "template category renamed",
    )))
}

async fn delete_template_category(
    State(state): State<ApiState>,
    Path(category_name): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state
        .context
        .template_store()
        .delete_category(&category_name)?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "category_name": category_name }),
        "template category deleted",
    )))
}

async fn list_templates(
    State(state): State<ApiState>,
    Query(query): Query<TemplateListQuery>,
) -> Result<Json<ApiResponse<Vec<TemplateSummary>>>, ApiError> {
    let templates = state
        .context
        .template_store()
        .list_templates(query.category.as_deref())?;
    Ok(Json(ApiResponse::ok(templates)))
}

async fn create_template(
    State(state): State<ApiState>,
    Json(request): Json<CreateTemplateRequest>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state.context.template_store().create_template(&request)?;
    Ok(Json(ApiResponse::with_message(
        document,
        "template created",
    )))
}

async fn get_template(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state
        .context
        .template_store()
        .get_template(&relative_path)?;
    Ok(Json(ApiResponse::ok(document)))
}

async fn update_template(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
    Json(request): Json<UpdateTemplateContentRequest>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state
        .context
        .template_store()
        .update_template(&relative_path, &request)?;
    Ok(Json(ApiResponse::with_message(
        document,
        "template updated",
    )))
}

async fn delete_template(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state
        .context
        .template_store()
        .delete_template(&relative_path)?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "relative_path": relative_path }),
        "template deleted",
    )))
}

async fn rename_template(
    State(state): State<ApiState>,
    Json(request): Json<RenameTemplateRequest>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state.context.template_store().rename_template(&request)?;
    Ok(Json(ApiResponse::with_message(
        document,
        "template renamed",
    )))
}

async fn copy_template(
    State(state): State<ApiState>,
    Json(request): Json<postpub_types::CopyTemplateRequest>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state.context.template_store().copy_template(&request)?;
    Ok(Json(ApiResponse::with_message(document, "template copied")))
}

async fn move_template(
    State(state): State<ApiState>,
    Json(request): Json<MoveTemplateRequest>,
) -> Result<Json<ApiResponse<TemplateDocument>>, ApiError> {
    let document = state.context.template_store().move_template(&request)?;
    Ok(Json(ApiResponse::with_message(document, "template moved")))
}

async fn list_articles(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<postpub_types::ArticleSummary>>>, ApiError> {
    let articles = state.context.article_store().list_articles()?;
    Ok(Json(ApiResponse::ok(articles)))
}

async fn get_article(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::ArticleDocument>>, ApiError> {
    let article = state.context.article_store().get_article(&relative_path)?;
    Ok(Json(ApiResponse::ok(article)))
}

async fn get_article_design(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
) -> Result<Json<ApiResponse<ArticleDesign>>, ApiError> {
    let design = state
        .context
        .article_store()
        .load_article_design(&relative_path)?;
    Ok(Json(ApiResponse::ok(design)))
}

async fn update_article(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
    Json(request): Json<UpdateArticleContentRequest>,
) -> Result<Json<ApiResponse<postpub_types::ArticleDocument>>, ApiError> {
    let article = state
        .context
        .article_store()
        .update_article(&relative_path, &request)?;
    Ok(Json(ApiResponse::with_message(article, "article updated")))
}

async fn save_article_design(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
    Json(design): Json<ArticleDesign>,
) -> Result<Json<ApiResponse<ArticleDesign>>, ApiError> {
    let saved = state
        .context
        .article_store()
        .save_article_design(&relative_path, &design)?;
    Ok(Json(ApiResponse::with_message(
        saved,
        "article design saved",
    )))
}

async fn delete_article(
    State(state): State<ApiState>,
    Path(relative_path): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state
        .context
        .article_store()
        .delete_article(&relative_path)?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "relative_path": relative_path }),
        "article deleted",
    )))
}

async fn create_generation_task(
    State(state): State<ApiState>,
    Json(request): Json<GenerateArticleRequest>,
) -> Result<Json<ApiResponse<postpub_types::GenerationTaskSummary>>, ApiError> {
    let summary = state
        .generation_manager
        .create_task(state.context.clone(), request)
        .await;
    Ok(Json(ApiResponse::with_message(
        summary,
        "generation task created",
    )))
}

async fn list_generation_tasks(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<postpub_types::GenerationTaskSummary>>>, ApiError> {
    let tasks = state.generation_manager.list_tasks().await;
    Ok(Json(ApiResponse::ok(tasks)))
}

async fn get_generation_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::GenerationTaskSummary>>, ApiError> {
    let Some(task) = state.generation_manager.get_task(&task_id).await else {
        return Err(ApiError::not_found(format!(
            "generation task not found: {task_id}"
        )));
    };
    Ok(Json(ApiResponse::ok(task)))
}

async fn delete_generation_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state.generation_manager.delete_task(&task_id).await?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "task_id": task_id }),
        "generation task deleted",
    )))
}

async fn delete_generation_tasks(
    State(state): State<ApiState>,
    Json(request): Json<DeleteTasksRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let deleted_ids = state.generation_manager.delete_tasks(&request.ids).await?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({
            "task_ids": deleted_ids,
            "deleted_count": deleted_ids.len()
        }),
        "generation tasks deleted",
    )))
}

async fn retry_generation_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::GenerationTaskSummary>>, ApiError> {
    let task = state
        .generation_manager
        .retry_task(state.context.clone(), &task_id)
        .await?;
    Ok(Json(ApiResponse::with_message(
        task,
        "generation task restarted",
    )))
}

async fn generation_events(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let Some(receiver) = state.generation_manager.subscribe(&task_id).await else {
        return Err(ApiError::not_found(format!(
            "generation task not found: {task_id}"
        )));
    };

    let stream = BroadcastStream::new(receiver).filter_map(|item| async move {
        match item {
            Ok(event) => {
                let payload = serde_json::to_string(&event).ok()?;
                Some(Ok(Event::default().data(payload)))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn create_publish_task(
    State(state): State<ApiState>,
    Json(request): Json<PublishArticleRequest>,
) -> Result<Json<ApiResponse<postpub_types::PublishTaskSummary>>, ApiError> {
    let summary = state
        .publish_manager
        .create_task(state.context.clone(), request)
        .await;
    Ok(Json(ApiResponse::with_message(
        summary,
        "publish task created",
    )))
}

async fn check_publish_target_login_status(
    State(state): State<ApiState>,
    Path(target_id): Path<String>,
) -> Result<Json<ApiResponse<PublishTargetLoginStatus>>, ApiError> {
    let status = state
        .context
        .publish_service()
        .check_target_login_status(&target_id)
        .await?;

    Ok(Json(ApiResponse::with_message(
        status,
        "publish target login status checked",
    )))
}

async fn list_publish_tasks(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<postpub_types::PublishTaskSummary>>>, ApiError> {
    let tasks = state.publish_manager.list_tasks().await;
    Ok(Json(ApiResponse::ok(tasks)))
}

async fn get_publish_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::PublishTaskSummary>>, ApiError> {
    let Some(task) = state.publish_manager.get_task(&task_id).await else {
        return Err(ApiError::not_found(format!(
            "publish task not found: {task_id}"
        )));
    };
    Ok(Json(ApiResponse::ok(task)))
}

async fn delete_publish_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    state.publish_manager.delete_task(&task_id).await?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({ "task_id": task_id }),
        "publish task deleted",
    )))
}

async fn delete_publish_tasks(
    State(state): State<ApiState>,
    Json(request): Json<DeleteTasksRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    let deleted_ids = state.publish_manager.delete_tasks(&request.ids).await?;
    Ok(Json(ApiResponse::with_message(
        serde_json::json!({
            "task_ids": deleted_ids,
            "deleted_count": deleted_ids.len()
        }),
        "publish tasks deleted",
    )))
}

async fn retry_publish_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::PublishTaskSummary>>, ApiError> {
    let task = state
        .publish_manager
        .retry_task(state.context.clone(), &task_id)
        .await?;
    Ok(Json(ApiResponse::with_message(
        task,
        "publish task restarted",
    )))
}

async fn cancel_publish_task(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<postpub_types::PublishTaskSummary>>, ApiError> {
    let task = state.publish_manager.cancel_task(&task_id).await?;
    Ok(Json(ApiResponse::with_message(
        task,
        "publish task canceled",
    )))
}

async fn publish_events(
    State(state): State<ApiState>,
    Path(task_id): Path<String>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let Some(receiver) = state.publish_manager.subscribe(&task_id).await else {
        return Err(ApiError::not_found(format!(
            "publish task not found: {task_id}"
        )));
    };

    let stream = BroadcastStream::new(receiver).filter_map(|item| async move {
        match item {
            Ok(event) => {
                let payload = serde_json::to_string(&event).ok()?;
                Some(Ok(Event::default().data(payload)))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
