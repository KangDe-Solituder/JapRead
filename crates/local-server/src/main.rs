use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};
type Database = japread_sources::Application;
async fn request(
    State(db): State<Database>,
    Path(op): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    // Development-only loopback adapter. Do not expose this as a hosted service.
    if headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| !["http://127.0.0.1:1420", "http://localhost:1420"].contains(&v))
    {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"来源不允许"})));
    }
    match tokio::task::spawn_blocking(move || db.dispatch(&op, payload)).await {
        Ok(Ok(value)) => (StatusCode::OK, Json(value)),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, Json(json!({"error":e}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":e.to_string()})),
        ),
    }
}
#[tokio::main]
async fn main() {
    std::fs::create_dir_all(".local").expect("创建数据目录");
    let db = japread_sources::Application::open(
        std::path::Path::new(".local/japread.sqlite3"),
        std::path::Path::new(".local/sources"),
        "development",
    )
    .expect("打开本地数据");
    let app = Router::new()
        .route("/api/{op}", post(request))
        .layer(DefaultBodyLimit::max(8_000_000))
        .with_state(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:1421")
        .await
        .expect("端口 1421 被占用");
    println!("JapRead 本地开发 API: http://127.0.0.1:1421；数据库 .local/japread.sqlite3");
    axum::serve(listener, app).await.unwrap();
}
