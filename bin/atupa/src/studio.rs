use rust_embed::RustEmbed;
use axum::{
    routing::get,
    response::{Html, IntoResponse, Response},
    http::{header, StatusCode, Uri},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(RustEmbed)]
#[folder = "../../studio/dist/"]
struct Asset;

pub struct StudioServer {
    report_json: Arc<Option<String>>,
}

impl StudioServer {
    pub fn new(report_json: Option<String>) -> Self {
        Self {
            report_json: Arc::new(report_json),
        }
    }

    pub async fn start(self, port: u16) -> anyhow::Result<()> {
        let report_data = self.report_json.clone();

        let app = Router::new()
            .route("/auto-load.json", get(move || {
                let data = report_data.clone();
                async move {
                    if let Some(json) = &*data {
                        Response::builder()
                            .header(header::CONTENT_TYPE, "application/json")
                            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                            .body(axum::body::Body::from(json.clone()))
                            .unwrap()
                            .into_response()
                    } else {
                        StatusCode::NOT_FOUND.into_response()
                    }
                }
            }))
            .fallback(static_handler);

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();

    if path.is_empty() || path == "index.html" {
        return index_html().await;
    }

    // Try to find the asset
    match Asset::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(axum::body::Body::from(content.data))
                .unwrap()
                .into_response()
        }
        None => {
            // If it's a sub-path, maybe it's an SPA route? 
            // Check if adding .html helps (unlikely for Vite but good for some)
            // For Vite SPA, we usually just return index.html for any non-asset route
            index_html().await
        }
    }
}

async fn index_html() -> Response {
    match Asset::get("index.html") {
        Some(content) => {
            Html(content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Studio assets not found. Did you run `npm run build` in the studio directory?").into_response(),
    }
}
