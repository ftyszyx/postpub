use std::{sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Html,
    routing::get,
    Router,
};
use postpub_api::build_router;
use postpub_core::AppContext;
use postpub_types::{
    ApiResponse, ArticleDesign, ArticleVariantDocument, ArticleVariantSummary, ConfigBundle,
    CustomLlmProvider, GenerateArticleRequest, GenerationTaskStatus, GenerationTaskSummary,
    PublishArticleRequest, PublishTaskStatus, PublishTaskSummary, TemplateDocument,
};
use tempfile::TempDir;
use tower::ServiceExt;

fn test_app() -> (Router, Arc<AppContext>, TempDir) {
    let temp = tempfile::tempdir().expect("temp dir");
    let context = Arc::new(AppContext::from_root("postpub-api", "0.1.0", temp.path()));
    context.bootstrap().expect("bootstrap");
    (build_router(context.clone()), context, temp)
}

async fn json_response<T: serde::de::DeserializeOwned>(
    app: &Router,
    request: Request<Body>,
) -> (StatusCode, T) {
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read body");
    let payload = serde_json::from_slice(&bytes).expect("json payload");
    (status, payload)
}

fn configure_mock_llm(context: &Arc<AppContext>, api_base: String) {
    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    bundle.ui_config.custom_llm_providers = vec![CustomLlmProvider {
        name: "Mock LLM".to_string(),
        api_key: "test-key".to_string(),
        api_base,
        model: "mock-model".to_string(),
        protocol_type: "openai".to_string(),
        enabled: true,
        ..CustomLlmProvider::default()
    }];
    context
        .config_store()
        .save_ui_config(&bundle.ui_config)
        .expect("save ui config");
}

async fn wait_for_task_completion(app: &Router, task_id: &str) -> GenerationTaskSummary {
    let mut final_task = None;
    for _ in 0..40 {
        let (status, task): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
            app,
            Request::builder()
                .uri(format!("/api/generation/tasks/{task_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        if matches!(
            task.data.status,
            GenerationTaskStatus::Succeeded | GenerationTaskStatus::Failed
        ) {
            final_task = Some(task.data);
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    final_task.expect("generation task completed")
}

async fn wait_for_publish_task_completion(app: &Router, task_id: &str) -> PublishTaskSummary {
    let mut final_task = None;
    for _ in 0..40 {
        let (status, task): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
            app,
            Request::builder()
                .uri(format!("/api/publish/tasks/{task_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        if matches!(
            task.data.status,
            PublishTaskStatus::Succeeded | PublishTaskStatus::Failed | PublishTaskStatus::Canceled
        ) {
            final_task = Some(task.data);
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    final_task.expect("publish task completed")
}

fn configure_invalid_wechat_publish_target(context: &Arc<AppContext>) {
    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    let target = bundle
        .config
        .publish_targets
        .iter_mut()
        .find(|item| item.id == "publish-wechat-1")
        .expect("wechat publish target");
    target.wechat.enable_reward = true;
    context
        .config_store()
        .save_config(&bundle.config)
        .expect("save config");
}

#[tokio::test]
async fn config_endpoints_roundtrip() {
    let (app, _, _temp) = test_app();

    let (status, mut bundle): (StatusCode, ApiResponse<ConfigBundle>) = json_response(
        &app,
        Request::builder()
            .uri("/api/config")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bundle.data.config.template_category, "general");
    assert_eq!(bundle.data.config.img_api.api_type, "picsum");

    bundle.data.config.publish_platform = "desktop".to_string();
    bundle.data.config.aiforge_search_min_results = 1;
    bundle.data.config.img_api.api_type = "ali".to_string();
    bundle.data.config.img_api.ali.api_key = "ali-key".to_string();
    bundle.data.config.img_api.ali.model = "wanx2.1-t2i-plus".to_string();
    bundle.data.config.publish_targets[0].wechat.cover_strategy = "custom_path".to_string();
    bundle.data.config.publish_targets[0].wechat.cover_path = "D:/covers/wechat.png".to_string();
    bundle.data.config.publish_targets[0]
        .wechat
        .declare_original = true;
    bundle.data.ui_config.custom_llm_providers[0].api_key = "llm-key".to_string();
    bundle.data.ui_config.custom_llm_providers[0].max_tokens = 999_999;

    let (status, saved): (StatusCode, ApiResponse<ConfigBundle>) = json_response(
        &app,
        Request::builder()
            .method("PUT")
            .uri("/api/config")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&bundle.data).expect("serialize config"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved.data.config.publish_platform, "desktop");
    assert_eq!(saved.data.config.aiforge_search_min_results, 1);
    assert_eq!(saved.data.config.img_api.api_type, "ali");
    assert_eq!(saved.data.config.img_api.ali.api_key, "ali-key");
    assert_eq!(
        saved.data.config.publish_targets[0].wechat.cover_strategy,
        "custom_path"
    );
    assert_eq!(
        saved.data.config.publish_targets[0].wechat.cover_path,
        "D:/covers/wechat.png"
    );
    assert!(saved.data.config.publish_targets[0].wechat.declare_original);
    assert_eq!(
        saved.data.ui_config.custom_llm_providers[0].api_key,
        bundle.data.ui_config.custom_llm_providers[0].api_key
    );
    assert_eq!(
        saved.data.ui_config.custom_llm_providers[0].max_tokens,
        131_072
    );
}

#[tokio::test]
async fn template_and_article_endpoints_work() {
    let (app, context, _temp) = test_app();

    let create_template_body = serde_json::json!({
        "name": "landing",
        "category": "campaign",
        "content": "<h1>{{title}}</h1><div>{{content}}</div>"
    });

    let (status, created): (StatusCode, ApiResponse<TemplateDocument>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/templates")
            .header("content-type", "application/json")
            .body(Body::from(create_template_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created.data.relative_path, "campaign/landing.html");

    let (status, template_list): (StatusCode, ApiResponse<Vec<postpub_types::TemplateSummary>>) =
        json_response(
            &app,
            Request::builder()
                .uri("/api/templates?category=campaign")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(template_list.data.len(), 1);

    context
        .article_store()
        .save_generated_article("Web|Rust Draft", "md", "# Title\n\nBody")
        .expect("save article");

    let (status, article_list): (StatusCode, ApiResponse<Vec<postpub_types::ArticleSummary>>) =
        json_response(
            &app,
            Request::builder()
                .uri("/api/articles")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(article_list.data.len(), 1);

    let relative_path = article_list.data[0].relative_path.clone();

    let update_article_body = serde_json::json!({
        "content": "# Updated\n\nMore body"
    });
    let (status, article): (StatusCode, ApiResponse<postpub_types::ArticleDocument>) =
        json_response(
            &app,
            Request::builder()
                .method("PUT")
                .uri(format!("/api/articles/{relative_path}"))
                .header("content-type", "application/json")
                .body(Body::from(update_article_body.to_string()))
                .expect("request"),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(article.data.preview_html.contains("<h1>Updated</h1>"));

    let design_body = serde_json::json!({
        "html": "<section>Canvas</section>",
        "css": "section { color: blue; }",
        "cover": "/images/cover.png"
    });
    let (status, design): (StatusCode, ApiResponse<ArticleDesign>) = json_response(
        &app,
        Request::builder()
            .method("PUT")
            .uri(format!("/api/articles/design/{relative_path}"))
            .header("content-type", "application/json")
            .body(Body::from(design_body.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(design.data.cover, "/images/cover.png");

    let (status, loaded_design): (StatusCode, ApiResponse<ArticleDesign>) = json_response(
        &app,
        Request::builder()
            .uri(format!("/api/articles/design/{relative_path}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(loaded_design.data.html, "<section>Canvas</section>");
}

#[tokio::test]
async fn generation_task_succeeds_with_reference_urls() {
    let (app, context, _temp) = test_app();

    let reference_app = Router::new().route(
        "/reference",
        get(|| async {
            Html(
                r#"<!DOCTYPE html>
                <html lang="en">
                  <head>
                    <title>Rust Workflow Reference</title>
                    <meta property="article:published_time" content="2026-03-28T08:00:00Z" />
                  </head>
                  <body>
                    <article>
                      <p>The shared Rust workflow keeps templates, articles, and generation in one core.</p>
                      <p>Reference extraction should return enough text for a deterministic draft.</p>
                    </article>
                  </body>
                </html>"#,
            )
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, reference_app)
            .await
            .expect("serve reference");
    });

    let llm_app = Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(|| async {
            axum::Json(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "# Rust workflow\n\n这是一篇用于测试的中文文章。"
                        }
                    }
                ]
            }))
        }),
    );
    let llm_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind llm listener");
    let llm_addr = llm_listener.local_addr().expect("llm addr");
    let llm_server = tokio::spawn(async move {
        axum::serve(llm_listener, llm_app).await.expect("serve llm");
    });

    configure_mock_llm(&context, format!("http://{llm_addr}/v1"));
    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    bundle.config.publish_targets.clear();
    context
        .config_store()
        .save_config(&bundle.config)
        .expect("save config");

    let reference_url = format!("http://{addr}/reference");
    let request = GenerateArticleRequest {
        topic: "Rust workflow".to_string(),
        reference_urls: vec![
            reference_url.clone(),
            reference_url.clone(),
            reference_url.clone(),
        ],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: true,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let task_id = created.data.id;
    let task = wait_for_task_completion(&app, &task_id).await;
    server.abort();
    llm_server.abort();
    assert_eq!(task.status, GenerationTaskStatus::Succeeded);
    let output = task.output.expect("generation output");
    assert_eq!(output.mode, "reference");
    assert_eq!(output.sources.len(), 3);
    assert!(output.preview_html.contains("Rust workflow"));
    assert!(output.article.is_some());
}

#[tokio::test]
async fn generation_task_succeeds_in_topic_mode_without_reference_urls() {
    let (app, context, _temp) = test_app();

    let llm_app = Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(|| async {
            axum::Json(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "# vicoding\n\n这是一篇用于测试纯主题生成模式的中文文章。"
                        }
                    }
                ]
            }))
        }),
    );
    let llm_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind llm listener");
    let llm_addr = llm_listener.local_addr().expect("llm addr");
    let llm_server = tokio::spawn(async move {
        axum::serve(llm_listener, llm_app).await.expect("serve llm");
    });

    configure_mock_llm(&context, format!("http://{llm_addr}/v1"));

    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    bundle.config.aiforge_search_min_results = 2;
    bundle.config.publish_targets.clear();
    bundle.aiforge_config.default_search_provider = "unknown-provider".to_string();
    bundle.aiforge_config.search.provider = "unknown-provider".to_string();
    context
        .config_store()
        .save_aiforge_config(&bundle.aiforge_config)
        .expect("save aiforge config");
    context
        .config_store()
        .save_config(&bundle.config)
        .expect("save config");

    let request = GenerateArticleRequest {
        topic: "vicoding".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let task_id = created.data.id;
    let task = wait_for_task_completion(&app, &task_id).await;
    llm_server.abort();

    assert_eq!(task.status, GenerationTaskStatus::Succeeded);
    assert!(task.error.is_none());
    let output = task.output.expect("generation output");
    assert_eq!(output.mode, "topic");
    assert!(output.sources.is_empty());
    assert!(output.preview_html.contains("vicoding"));
    assert!(output.article.is_none());
}

#[tokio::test]
async fn generation_task_fails_when_llm_api_key_is_missing() {
    let (app, _context, _temp) = test_app();

    let request = GenerateArticleRequest {
        topic: "vicoding".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let task = wait_for_task_completion(&app, &created.data.id).await;
    assert_eq!(task.status, GenerationTaskStatus::Failed);
    assert!(task
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("fill api_key"));
}

#[tokio::test]
async fn generation_task_surfaces_detailed_llm_error_messages() {
    let (app, context, _temp) = test_app();

    let llm_app = Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(|| async {
            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({
                    "error": {
                        "code": "InvalidParameter",
                        "message": "The parameter `max_tokens` specified in the request are not valid.",
                        "param": "max_tokens",
                        "type": "BadRequest"
                    }
                })),
            )
        }),
    );
    let llm_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind llm listener");
    let llm_addr = llm_listener.local_addr().expect("llm addr");
    let llm_server = tokio::spawn(async move {
        axum::serve(llm_listener, llm_app).await.expect("serve llm");
    });

    configure_mock_llm(&context, format!("http://{llm_addr}/v1"));

    let request = GenerateArticleRequest {
        topic: "vicoding".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let task = wait_for_task_completion(&app, &created.data.id).await;
    llm_server.abort();

    assert_eq!(task.status, GenerationTaskStatus::Failed);
    assert!(task
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("The parameter `max_tokens` specified in the request are not valid."));
}

#[tokio::test]
async fn generation_tasks_are_restored_after_restart() {
    let (app, context, temp) = test_app();

    let request = GenerateArticleRequest {
        topic: "vicoding".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let finished = wait_for_task_completion(&app, &created.data.id).await;
    assert_eq!(finished.status, GenerationTaskStatus::Failed);

    drop(app);
    drop(context);

    let restarted_context = Arc::new(AppContext::from_root("postpub-api", "0.1.0", temp.path()));
    restarted_context.bootstrap().expect("bootstrap restart");
    let restarted_app = build_router(restarted_context);

    let (status, tasks): (StatusCode, ApiResponse<Vec<GenerationTaskSummary>>) = json_response(
        &restarted_app,
        Request::builder()
            .uri("/api/generation/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.data.len(), 1);
    assert_eq!(tasks.data[0].id, finished.id);
    assert_eq!(tasks.data[0].status, GenerationTaskStatus::Failed);
}

#[tokio::test]
async fn generation_task_retry_reuses_existing_task_id() {
    let (app, context, _temp) = test_app();

    let request = GenerateArticleRequest {
        topic: "vicoding".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let failed = wait_for_task_completion(&app, &created.data.id).await;
    assert_eq!(failed.status, GenerationTaskStatus::Failed);

    let llm_app = Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(|| async {
            axum::Json(serde_json::json!({
                "choices": [
                    {
                        "message": {
                            "content": "# vicoding\n\n这是一篇用于测试同任务重试的中文文章。"
                        }
                    }
                ]
            }))
        }),
    );
    let llm_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind llm listener");
    let llm_addr = llm_listener.local_addr().expect("llm addr");
    let llm_server = tokio::spawn(async move {
        axum::serve(llm_listener, llm_app).await.expect("serve llm");
    });

    configure_mock_llm(&context, format!("http://{llm_addr}/v1"));
    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    bundle.config.publish_targets.clear();
    context
        .config_store()
        .save_config(&bundle.config)
        .expect("save config");

    let (status, retried): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/generation/tasks/{}/retry", failed.id))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried.data.id, failed.id);

    let succeeded = wait_for_task_completion(&app, &failed.id).await;
    llm_server.abort();

    assert_eq!(succeeded.status, GenerationTaskStatus::Succeeded);
    let output = succeeded.output.expect("generation output");
    assert_eq!(output.mode, "topic");

    let (status, tasks): (StatusCode, ApiResponse<Vec<GenerationTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/generation/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.data.len(), 1);
    assert_eq!(tasks.data[0].id, failed.id);
}

#[tokio::test]
async fn generation_task_can_be_deleted_after_completion() {
    let (app, _context, _temp) = test_app();

    let request = GenerateArticleRequest {
        topic: "delete generation task".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let failed = wait_for_task_completion(&app, &created.data.id).await;
    assert_eq!(failed.status, GenerationTaskStatus::Failed);

    let (status, deleted): (StatusCode, ApiResponse<serde_json::Value>) = json_response(
        &app,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/generation/tasks/{}", failed.id))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted.data["task_id"].as_str(), Some(failed.id.as_str()));

    let (status, tasks): (StatusCode, ApiResponse<Vec<GenerationTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/generation/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(tasks.data.is_empty());
}

#[tokio::test]
async fn generation_tasks_can_be_deleted_in_batch() {
    let (app, _context, _temp) = test_app();

    let request = GenerateArticleRequest {
        topic: "batch delete generation tasks".to_string(),
        reference_urls: vec![],
        template_category: Some("general".to_string()),
        template_name: Some("magazine".to_string()),
        save_output: false,
    };

    let mut task_ids = Vec::new();
    for _ in 0..2 {
        let (status, created): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/api/generation/tasks")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&request).expect("serialize request"),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let failed = wait_for_task_completion(&app, &created.data.id).await;
        assert_eq!(failed.status, GenerationTaskStatus::Failed);
        task_ids.push(failed.id);
    }

    let (status, deleted): (StatusCode, ApiResponse<serde_json::Value>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/generation/tasks/actions/delete")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "ids": task_ids }).to_string(),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted.data["deleted_count"].as_u64(), Some(2));

    let (status, tasks): (StatusCode, ApiResponse<Vec<GenerationTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/generation/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(tasks.data.is_empty());
}

#[tokio::test]
async fn publish_task_fails_with_clear_wechat_validation_error() {
    let (app, context, _temp) = test_app();
    configure_invalid_wechat_publish_target(&context);

    let article = context
        .article_store()
        .save_generated_source_article(
            "Wechat Publish Demo",
            "# Wechat Publish Demo\n\nBody",
            &[ArticleVariantDocument {
                summary: ArticleVariantSummary {
                    target_id: "publish-wechat-1".to_string(),
                    target_name: "微信公众号 1".to_string(),
                    platform_type: "wechat".to_string(),
                    format: "HTML".to_string(),
                    size_bytes: 0,
                    updated_at: chrono::Utc::now(),
                },
                content: "<section><h1>Wechat Publish Demo</h1><p>Body</p></section>".to_string(),
                preview_html: "<section><h1>Wechat Publish Demo</h1><p>Body</p></section>"
                    .to_string(),
            }],
        )
        .expect("save article");

    let request = PublishArticleRequest {
        article_relative_path: article.summary.relative_path.clone(),
        target_id: "publish-wechat-1".to_string(),
        mode: "draft".to_string(),
    };

    let (status, created): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/publish/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize publish request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let finished = wait_for_publish_task_completion(&app, &created.data.id).await;
    assert_eq!(finished.status, PublishTaskStatus::Failed);
    assert!(finished
        .error
        .as_deref()
        .unwrap_or_default()
        .contains("wechat reward automation is not implemented yet"));
    assert!(finished.events.iter().any(|event| event.stage == "prepare"));
    assert!(finished
        .events
        .iter()
        .any(|event| event.stage == "wechat.prepare"));
}

#[tokio::test]
async fn publish_task_retry_reuses_existing_task_id() {
    let (app, context, _temp) = test_app();
    configure_invalid_wechat_publish_target(&context);

    let article = context
        .article_store()
        .save_generated_source_article(
            "Wechat Publish Retry",
            "# Wechat Publish Retry\n\nBody",
            &[ArticleVariantDocument {
                summary: ArticleVariantSummary {
                    target_id: "publish-wechat-1".to_string(),
                    target_name: "微信公众号 1".to_string(),
                    platform_type: "wechat".to_string(),
                    format: "HTML".to_string(),
                    size_bytes: 0,
                    updated_at: chrono::Utc::now(),
                },
                content: "<section><h1>Wechat Publish Retry</h1><p>Body</p></section>".to_string(),
                preview_html: "<section><h1>Wechat Publish Retry</h1><p>Body</p></section>"
                    .to_string(),
            }],
        )
        .expect("save article");

    let request = PublishArticleRequest {
        article_relative_path: article.summary.relative_path.clone(),
        target_id: "publish-wechat-1".to_string(),
        mode: "draft".to_string(),
    };

    let (status, created): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/publish/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize publish request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let failed = wait_for_publish_task_completion(&app, &created.data.id).await;
    assert_eq!(failed.status, PublishTaskStatus::Failed);

    let (status, retried): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/publish/tasks/{}/retry", failed.id))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried.data.id, failed.id);

    let failed_again = wait_for_publish_task_completion(&app, &failed.id).await;
    assert_eq!(failed_again.status, PublishTaskStatus::Failed);

    let (status, tasks): (StatusCode, ApiResponse<Vec<PublishTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/publish/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.data.len(), 1);
    assert_eq!(tasks.data[0].id, failed.id);
}

#[tokio::test]
async fn publish_task_can_be_deleted_after_completion() {
    let (app, context, _temp) = test_app();
    configure_invalid_wechat_publish_target(&context);

    let article = context
        .article_store()
        .save_generated_source_article(
            "Wechat Publish Delete",
            "# Wechat Publish Delete\n\nBody",
            &[ArticleVariantDocument {
                summary: ArticleVariantSummary {
                    target_id: "publish-wechat-1".to_string(),
                    target_name: "微信公众号 1".to_string(),
                    platform_type: "wechat".to_string(),
                    format: "HTML".to_string(),
                    size_bytes: 0,
                    updated_at: chrono::Utc::now(),
                },
                content: "<section><h1>Wechat Publish Delete</h1><p>Body</p></section>".to_string(),
                preview_html: "<section><h1>Wechat Publish Delete</h1><p>Body</p></section>"
                    .to_string(),
            }],
        )
        .expect("save article");

    let request = PublishArticleRequest {
        article_relative_path: article.summary.relative_path.clone(),
        target_id: "publish-wechat-1".to_string(),
        mode: "draft".to_string(),
    };

    let (status, created): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/publish/tasks")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&request).expect("serialize publish request"),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let failed = wait_for_publish_task_completion(&app, &created.data.id).await;
    assert_eq!(failed.status, PublishTaskStatus::Failed);

    let (status, deleted): (StatusCode, ApiResponse<serde_json::Value>) = json_response(
        &app,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/publish/tasks/{}", failed.id))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted.data["task_id"].as_str(), Some(failed.id.as_str()));

    let (status, tasks): (StatusCode, ApiResponse<Vec<PublishTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/publish/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(tasks.data.is_empty());
}

#[tokio::test]
async fn publish_tasks_can_be_deleted_in_batch() {
    let (app, context, _temp) = test_app();
    configure_invalid_wechat_publish_target(&context);

    let article = context
        .article_store()
        .save_generated_source_article(
            "Wechat Publish Batch Delete",
            "# Wechat Publish Batch Delete\n\nBody",
            &[ArticleVariantDocument {
                summary: ArticleVariantSummary {
                    target_id: "publish-wechat-1".to_string(),
                    target_name: "微信公众号 1".to_string(),
                    platform_type: "wechat".to_string(),
                    format: "HTML".to_string(),
                    size_bytes: 0,
                    updated_at: chrono::Utc::now(),
                },
                content: "<section><h1>Wechat Publish Batch Delete</h1><p>Body</p></section>"
                    .to_string(),
                preview_html: "<section><h1>Wechat Publish Batch Delete</h1><p>Body</p></section>"
                    .to_string(),
            }],
        )
        .expect("save article");

    let request = PublishArticleRequest {
        article_relative_path: article.summary.relative_path.clone(),
        target_id: "publish-wechat-1".to_string(),
        mode: "draft".to_string(),
    };

    let mut task_ids = Vec::new();
    for _ in 0..2 {
        let (status, created): (StatusCode, ApiResponse<PublishTaskSummary>) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/api/publish/tasks")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&request).expect("serialize publish request"),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let failed = wait_for_publish_task_completion(&app, &created.data.id).await;
        assert_eq!(failed.status, PublishTaskStatus::Failed);
        task_ids.push(failed.id);
    }

    let (status, deleted): (StatusCode, ApiResponse<serde_json::Value>) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/publish/tasks/actions/delete")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({ "ids": task_ids }).to_string(),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted.data["deleted_count"].as_u64(), Some(2));

    let (status, tasks): (StatusCode, ApiResponse<Vec<PublishTaskSummary>>) = json_response(
        &app,
        Request::builder()
            .uri("/api/publish/tasks")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(tasks.data.is_empty());
}
