use axum::{
    async_trait,
    extract::{FromRequest, Request},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;

pub struct ValidatedJson<T>(pub T);

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        // 1. 检查 Content-Type
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("application/json") {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(serde_json::json!({
                    "error": "unsupported_media_type",
                    "message": "Content-Type must be application/json"
                })),
            )
                .into_response());
        }

        // 2. 读取 body 字节
        let bytes = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
            Ok(b) => b,
            Err(_) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "empty_body"})),
                )
                    .into_response());
            }
        };

        if bytes.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "empty_body"})),
            )
                .into_response());
        }

        // 3. 先验证 JSON 语法
        if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "malformed_json",
                    "message": "request body is not valid JSON"
                })),
            )
                .into_response());
        }

        // 4. 反序列化为目标类型
        match serde_json::from_slice::<T>(&bytes) {
            Ok(value) => Ok(ValidatedJson(value)),
            Err(e) => Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "invalid_body",
                    "message": e.to_string()
                })),
            )
                .into_response()),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{routing::post, Router};
    use axum_test::TestServer;
    use serde::Deserialize;

    use super::ValidatedJson;

    #[derive(Deserialize)]
    struct TestPayload {
        name: String,
    }

    async fn test_handler(
        ValidatedJson(payload): ValidatedJson<TestPayload>,
    ) -> axum::Json<serde_json::Value> {
        axum::Json(serde_json::json!({"name": payload.name}))
    }

    fn test_router() -> Router {
        Router::new().route("/test", post(test_handler))
    }

    #[tokio::test]
    async fn valid_json_passes_through() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .json(&serde_json::json!({"name": "hello"}))
            .await;
        assert_eq!(resp.status_code(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn malformed_json_returns_400_with_error_field() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("application/json")
            .bytes(axum::body::Bytes::from_static(b"{ not valid json"))
            .await;
        assert_eq!(resp.status_code(), axum::http::StatusCode::BAD_REQUEST);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "malformed_json");
        assert!(body.get("request_id").is_none());
    }

    #[tokio::test]
    async fn empty_body_returns_400() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("application/json")
            .bytes(axum::body::Bytes::new())
            .await;
        assert_eq!(resp.status_code(), axum::http::StatusCode::BAD_REQUEST);
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "empty_body");
    }

    #[tokio::test]
    async fn wrong_content_type_returns_415() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .content_type("text/plain")
            .bytes(axum::body::Bytes::from_static(b"{\"name\":\"hello\"}"))
            .await;
        assert_eq!(
            resp.status_code(),
            axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "unsupported_media_type");
    }

    #[tokio::test]
    async fn type_mismatch_returns_422() {
        let server = TestServer::new(test_router()).unwrap();
        let resp = server
            .post("/test")
            .json(&serde_json::json!({"name": 123}))
            .await;
        assert_eq!(
            resp.status_code(),
            axum::http::StatusCode::UNPROCESSABLE_ENTITY
        );
        let body: serde_json::Value = resp.json();
        assert_eq!(body["error"], "invalid_body");
    }
}
