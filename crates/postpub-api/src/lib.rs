mod error;
mod routes;
mod state;

use std::{path::PathBuf, sync::Arc};

use axum::Router;
use postpub_core::AppContext;
use tower_http::services::{ServeDir, ServeFile};

pub use state::ApiState;

pub fn build_router(context: Arc<AppContext>) -> Router {
    let state = ApiState::new(context.clone());
    let dist_dir = frontend_dist_dir();
    let index_file = dist_dir.join("index.html");
    let images_dir = context.paths().images_dir();

    routes::api_router()
        .nest_service("/images", ServeDir::new(images_dir))
        .fallback_service(ServeDir::new(dist_dir).not_found_service(ServeFile::new(index_file)))
        .with_state(state)
}

fn frontend_dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("frontend")
        .join("dist")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::Body, http::Request};
    use postpub_core::AppContext;
    use tower::ServiceExt;

    use crate::build_router;

    #[tokio::test]
    async fn serves_health_endpoint() {
        let context = Arc::new(AppContext::new("postpub-api", "0.1.0"));
        context.bootstrap().expect("bootstrap");
        let app = build_router(context);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/system/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), 200);
    }
}
