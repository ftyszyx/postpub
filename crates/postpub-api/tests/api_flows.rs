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
    ApiResponse, ArticleDesign, ConfigBundle, GenerateArticleRequest, GenerationTaskStatus,
    GenerationTaskSummary, TemplateDocument,
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
    let (app, _context, _temp) = test_app();

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

    let reference_url = format!("http://{addr}/reference");
    let request = GenerateArticleRequest {
        topic: "Rust workflow".to_string(),
        platform: "web".to_string(),
        reference_urls: vec![
            reference_url.clone(),
            reference_url.clone(),
            reference_url.clone(),
        ],
        reference_ratio: 0.4,
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
    let mut final_task = None;
    for _ in 0..40 {
        let (status, task): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
            &app,
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

    server.abort();

    let task = final_task.expect("generation task completed");
    assert_eq!(task.status, GenerationTaskStatus::Succeeded);
    let output = task.output.expect("generation output");
    assert_eq!(output.mode, "reference");
    assert_eq!(output.sources.len(), 3);
    assert!(output.preview_html.contains("Rust workflow"));
    assert!(output.article.is_some());
}

#[tokio::test]
async fn generation_task_falls_back_when_search_provider_is_unavailable() {
    let (app, context, _temp) = test_app();

    let mut bundle = context.config_store().load_bundle().expect("load bundle");
    bundle.config.aiforge_search_min_results = 2;
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
        platform: String::new(),
        reference_urls: vec![],
        reference_ratio: 0.0,
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
    let mut final_task = None;
    for _ in 0..40 {
        let (status, task): (StatusCode, ApiResponse<GenerationTaskSummary>) = json_response(
            &app,
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

    let task = final_task.expect("generation task completed");
    assert_eq!(task.status, GenerationTaskStatus::Succeeded);
    let output = task.output.expect("generation output");
    assert_eq!(output.mode, "fallback");
    assert_eq!(output.sources.len(), 2);
    assert!(output.content.contains("External news retrieval is unavailable"));
}
